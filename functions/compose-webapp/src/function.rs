//! Composes a Deployment and a Service for a WebApp.

use std::collections::BTreeMap;

use crossplane_models::com::example::platform::v1alpha1::WebApp;
use crossplane_models::io::k8s::api::apps::v1::{Deployment, DeploymentSpec};
use crossplane_models::io::k8s::api::core::v1::{
    Container, ContainerPort, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec,
};
use crossplane_models::io::k8s::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use crossplane_models::io::k8s::apimachinery::pkg::util::intstr::IntOrString;
use function_sdk_rust::proto::v1::function_runner_service_server::FunctionRunnerService;
use function_sdk_rust::proto::v1::{RunFunctionRequest, RunFunctionResponse};
use function_sdk_rust::{resource, response};
use tonic::{Request, Response, Status};

/// The composition function.
#[derive(Debug, Default)]
pub struct Function;

#[tonic::async_trait]
impl FunctionRunnerService for Function {
    async fn run_function(
        &self,
        request: Request<RunFunctionRequest>,
    ) -> Result<Response<RunFunctionResponse>, Status> {
        let req = request.into_inner();
        let tag = req.meta.as_ref().map(|m| m.tag.clone()).unwrap_or_default();
        tracing::info!(tag, "running function");

        let mut rsp = response::to(&req, response::DEFAULT_TTL);

        let observed = req.observed.as_ref().and_then(|s| s.composite.as_ref());
        let xr: WebApp = match resource::get(observed) {
            Ok(xr) => xr,
            Err(e) => {
                response::fatal(&mut rsp, format!("cannot get xr: {e}"));
                return Ok(Response::new(rsp));
            }
        };

        let metadata = xr.metadata.unwrap_or_default();
        let spec = xr.spec.unwrap_or_default();
        let (Some(name), Some(image)) = (metadata.name, spec.image) else {
            response::fatal(&mut rsp, "xr is missing metadata.name or spec.image");
            return Ok(Response::new(rsp));
        };

        let ports = spec.ports.unwrap_or_default();
        let labels = BTreeMap::from([("app.kubernetes.io/name".to_string(), name.clone())]);

        // Build each resource from its generated model. Default fills in the
        // apiVersion and kind, and leaves every field the function doesn't set
        // out of the desired state.
        let deployment = Deployment {
            metadata: Some(ObjectMeta {
                name: Some(name.clone()),
                namespace: metadata.namespace.clone(),
                labels: Some(labels.clone()),
                ..Default::default()
            }),
            spec: Some(DeploymentSpec {
                replicas: spec.replicas.map(|r| r as i32),
                selector: Some(LabelSelector {
                    match_labels: Some(labels.clone()),
                    ..Default::default()
                }),
                template: Some(PodTemplateSpec {
                    metadata: Some(ObjectMeta {
                        labels: Some(labels.clone()),
                        ..Default::default()
                    }),
                    spec: Some(PodSpec {
                        containers: Some(vec![Container {
                            name: Some(name.clone()),
                            image: Some(image),
                            ports: Some(
                                ports
                                    .iter()
                                    .map(|p| ContainerPort {
                                        container_port: Some(*p as i32),
                                        ..Default::default()
                                    })
                                    .collect(),
                            ),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let service = Service {
            metadata: Some(ObjectMeta {
                name: Some(name),
                namespace: metadata.namespace,
                ..Default::default()
            }),
            spec: Some(ServiceSpec {
                selector: Some(labels),
                ports: Some(
                    ports
                        .iter()
                        .map(|p| ServicePort {
                            protocol: Some("TCP".to_string()),
                            port: Some(*p as i32),
                            target_port: Some(IntOrString::Int(*p)),
                            ..Default::default()
                        })
                        .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        let desired = rsp.desired.get_or_insert_default();
        resource::update(
            desired
                .resources
                .entry("deployment".to_string())
                .or_default(),
            &deployment,
        )
        .map_err(|e| Status::internal(e.to_string()))?;
        resource::update(
            desired.resources.entry("service".to_string()).or_default(),
            &service,
        )
        .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(rsp))
    }
}