// Operator grant policy of the demo runtime (--sandbox-policy-file): nothing
// is granted.
//
// function-wasm evaluates it default-deny and a forbid always wins, so every
// forbid below also holds against a permit added later. A plugin's manifest
// can only ask for a capability, a Composition's compositionPolicy can only
// narrow, and this file is the only place that can grant one. A plugin that
// asks for nothing, like pdb.wasm, still runs: it gets the request and the
// host's log import, and that is all.

// No private /tmp.
forbid (principal, action == Action::"usePrivateTmp", resource);

// No environment variables bound to step credentials, and no credential spent.
forbid (principal, action == Action::"setEnv", resource);
forbid (principal, action == Action::"spendCredential", resource);

// No HTTP egress to any host, and no address may be dialled. Scoped with ==
// on purpose: the dial block list is compiled from rules that name
// dialAddress, so a blanket forbid on every action would not reach it.
forbid (principal, action == Action::"grantEgress", resource);
forbid (principal, action == Action::"dialAddress", resource);

// pullModule is decided by a Composition's compositionPolicy; the operator
// layer never evaluates it, so this line only states the intent.
forbid (principal, action == Action::"pullModule", resource);

// requireSignature is left out deliberately. It works the other way round: a
// permit demands a cosign signature for a repository, so a forbid here would
// mean "never demand one".
