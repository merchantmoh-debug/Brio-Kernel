//! gRPC protocol definitions for mesh transport.
//!
//! This module contains the generated protobuf code for inter-node communication
//! in the Brio service mesh, including message routing and health checks.

#![allow(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::too_many_lines)]
tonic::include_proto!("mesh");
