//! A function-wasm plugin (ABI v2 component) that adds a PodDisruptionBudget
//! for the Deployment an earlier pipeline step put in the desired state. It
//! reads and writes the resources through the project's generated models.

use std::collections::BTreeMap;

use crossplane_models::io::k8s::api::apps::v1::Deployment;
use crossplane_models::io::k8s::api::policy::v1::{PodDisruptionBudget, PodDisruptionBudgetSpec};
use crossplane_models::io::k8s::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use crossplane_models::io::k8s::apimachinery::pkg::util::intstr::IntOrString;
use prost::Message;
use prost_types::value::Kind;
use prost_types::{Duration, ListValue, Struct, Value};

#[cfg(target_arch = "wasm32")]
pub mod bindings;

/// The crossplane `RunFunction` messages.
pub mod fnv1 {
    // prost copies the proto comments verbatim; their list formatting is not
    // rustdoc's.
    #![allow(clippy::doc_lazy_continuation)]
    // `fn` is a Rust keyword, so prost escapes that segment of the proto package.
    include!(concat!(env!("OUT_DIR"), "/apiextensions.r#fn.proto.v1.rs"));
}

use fnv1::{
    Resource, ResponseMeta, Result as FnResult, RunFunctionRequest, RunFunctionResponse, Severity,
    Target,
};

/// The desired-state names of the Deployment to protect and of the budget.
const DEPLOYMENT: &str = "deployment";
const BUDGET: &str = "pod-disruption-budget";

const TTL_SECONDS: i64 = 60;

/// Adds a PodDisruptionBudget selecting the desired Deployment's pods. The
/// rest of the desired state and the context pass through unchanged.
pub fn run_function(req: &RunFunctionRequest) -> Result<RunFunctionResponse, String> {
    let mut desired = req.desired.clone().unwrap_or_default();

    let deployment: Deployment = desired
        .resources
        .get(DEPLOYMENT)
        .and_then(|r| r.resource.as_ref())
        .ok_or(format!(
            "no {DEPLOYMENT:?} in the desired state: run this plugin after the step that composes it"
        ))
        .and_then(from_struct)?;

    let metadata = deployment.metadata.unwrap_or_default();
    let selector = deployment
        .spec
        .and_then(|s| s.selector)
        .ok_or(format!("desired {DEPLOYMENT:?} has no spec.selector"))?;

    let budget = PodDisruptionBudget {
        metadata: Some(ObjectMeta {
            name: metadata.name.clone(),
            namespace: metadata.namespace,
            ..Default::default()
        }),
        spec: Some(PodDisruptionBudgetSpec {
            max_unavailable: Some(IntOrString::Int(1)),
            selector: Some(selector),
            ..Default::default()
        }),
        ..Default::default()
    };
    desired.resources.insert(
        BUDGET.to_string(),
        Resource {
            resource: Some(to_struct(&budget)?),
            ..Default::default()
        },
    );

    let name = metadata.name.unwrap_or_default();
    log::info("Added PodDisruptionBudget", &[("deployment", &name)]);

    Ok(RunFunctionResponse {
        meta: Some(response_meta(req)),
        desired: Some(desired),
        context: req.context.clone(),
        results: vec![FnResult {
            severity: Severity::Normal as i32,
            message: format!("added a PodDisruptionBudget for Deployment {name}"),
            target: Some(Target::Composite as i32),
            ..Default::default()
        }],
        ..Default::default()
    })
}

/// Decode, run, encode. A failure becomes a fatal result.
pub fn handle(input: &[u8]) -> Vec<u8> {
    let rsp = match RunFunctionRequest::decode(input) {
        Ok(req) => run_function(&req).unwrap_or_else(|e| fatal(Some(&req), &e)),
        Err(e) => fatal(None, &format!("cannot decode RunFunctionRequest: {e}")),
    };
    rsp.encode_to_vec()
}

fn response_meta(req: &RunFunctionRequest) -> ResponseMeta {
    ResponseMeta {
        tag: req.meta.as_ref().map(|m| m.tag.clone()).unwrap_or_default(),
        ttl: Some(Duration {
            seconds: TTL_SECONDS,
            nanos: 0,
        }),
    }
}

fn fatal(req: Option<&RunFunctionRequest>, msg: &str) -> RunFunctionResponse {
    RunFunctionResponse {
        meta: req.map(response_meta),
        results: vec![FnResult {
            severity: Severity::Fatal as i32,
            message: msg.to_string(),
            target: Some(Target::Composite as i32),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Reads a protobuf Struct as a generated model.
fn from_struct<T: serde::de::DeserializeOwned>(s: &Struct) -> Result<T, String> {
    serde_json::from_value(struct_to_json(s)).map_err(|e| e.to_string())
}

/// Writes a generated model as a protobuf Struct.
fn to_struct<T: serde::Serialize>(model: &T) -> Result<Struct, String> {
    match json_to_value(serde_json::to_value(model).map_err(|e| e.to_string())?).kind {
        Some(Kind::StructValue(s)) => Ok(s),
        _ => Err("model does not serialize to an object".to_string()),
    }
}

fn struct_to_json(s: &Struct) -> serde_json::Value {
    serde_json::Value::Object(
        s.fields
            .iter()
            .map(|(k, v)| (k.clone(), value_to_json(v)))
            .collect(),
    )
}

fn value_to_json(v: &Value) -> serde_json::Value {
    match &v.kind {
        None | Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => (*b).into(),
        // A Struct number is always a double, and an integer field of a model
        // does not deserialize from 3.0.
        Some(Kind::NumberValue(n)) if n.fract() == 0.0 && n.abs() < 9e15 => (*n as i64).into(),
        Some(Kind::NumberValue(n)) => (*n).into(),
        Some(Kind::StringValue(s)) => s.clone().into(),
        Some(Kind::ListValue(l)) => l.values.iter().map(value_to_json).collect(),
        Some(Kind::StructValue(s)) => struct_to_json(s),
    }
}

fn json_to_value(v: serde_json::Value) -> Value {
    let kind = match v {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(b) => Kind::BoolValue(b),
        serde_json::Value::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or_default()),
        serde_json::Value::String(s) => Kind::StringValue(s),
        serde_json::Value::Array(a) => Kind::ListValue(ListValue {
            values: a.into_iter().map(json_to_value).collect(),
        }),
        serde_json::Value::Object(o) => Kind::StructValue(Struct {
            fields: o
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect::<BTreeMap<_, _>>(),
        }),
    };
    Value { kind: Some(kind) }
}

/// Structured logging through the host: the world's `log` import on the wasm
/// target, stderr natively.
pub mod log {
    #[cfg(target_arch = "wasm32")]
    pub fn info(msg: &str, kv: &[(&str, &str)]) {
        let kv: Vec<(String, String)> = kv
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        crate::bindings::log(crate::bindings::LogLevel::Info, msg, &kv);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn info(msg: &str, kv: &[(&str, &str)]) {
        eprintln!("{msg} {kv:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossplane_models::io::k8s::api::apps::v1::DeploymentSpec;
    use crossplane_models::io::k8s::apimachinery::pkg::apis::meta::v1::LabelSelector;

    fn request_with(resources: Vec<(&str, Struct)>) -> RunFunctionRequest {
        RunFunctionRequest {
            desired: Some(fnv1::State {
                resources: resources
                    .into_iter()
                    .map(|(name, s)| {
                        let resource = Resource {
                            resource: Some(s),
                            ..Default::default()
                        };
                        (name.to_string(), resource)
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn adds_a_budget_selecting_the_deployment() {
        let labels = BTreeMap::from([("app.kubernetes.io/name".to_string(), "podinfo".to_string())]);
        let deployment = Deployment {
            metadata: Some(ObjectMeta {
                name: Some("podinfo".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            }),
            spec: Some(DeploymentSpec {
                replicas: Some(3),
                selector: Some(LabelSelector {
                    match_labels: Some(labels.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let req = request_with(vec![(DEPLOYMENT, to_struct(&deployment).unwrap())]);

        let rsp = run_function(&req).unwrap();
        let desired = rsp.desired.unwrap();
        assert!(desired.resources.contains_key(DEPLOYMENT));

        let got = desired.resources[BUDGET].resource.as_ref().unwrap();
        let budget: PodDisruptionBudget = from_struct(got).unwrap();
        assert_eq!(budget.api_version.as_deref(), Some("policy/v1"));
        assert_eq!(budget.kind.as_deref(), Some("PodDisruptionBudget"));
        let metadata = budget.metadata.unwrap();
        assert_eq!(metadata.name.as_deref(), Some("podinfo"));
        assert_eq!(metadata.namespace.as_deref(), Some("default"));
        let spec = budget.spec.unwrap();
        assert_eq!(spec.selector.unwrap().match_labels, Some(labels));
        assert!(matches!(spec.max_unavailable, Some(IntOrString::Int(1))));
    }

    #[test]
    fn without_a_deployment_the_result_is_fatal() {
        let req = request_with(vec![]);
        let rsp = RunFunctionResponse::decode(handle(&req.encode_to_vec()).as_slice()).unwrap();
        assert_eq!(rsp.results[0].severity, Severity::Fatal as i32);
        assert!(rsp.results[0].message.contains("no \"deployment\""));
    }
}
