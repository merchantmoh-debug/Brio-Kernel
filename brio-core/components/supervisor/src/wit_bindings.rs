//! WIT Bindings Facade
//!
//! This module provides a clean abstraction over the generated WIT bindings.
//! It re-exports only what the supervisor needs, hiding generated code details.

// We'll use manual types that mirror the WIT interface for now.
// The actual wit_bindgen::generate! macro call is in lib.rs.

/// SQL State interface bindings.
pub mod sql_state {
    /// Row returned from a SQL query.
    #[derive(Debug, Clone)]
    pub struct Row {
        /// Column names for this row.
        pub columns: Vec<String>,
        /// Values corresponding to each column.
        pub values: Vec<String>,
    }

    /// Execute a SQL query that returns rows.
    ///
    /// # Errors
    /// Returns error string if query fails.
    pub fn query(sql: &str, params: &[String]) -> Result<Vec<Row>, String> {
        // This calls into the generated bindings
        #[cfg(target_arch = "wasm32")]
        {
            use crate::brio_host::sql_state as wit;

            let result = wit::query(sql, params)?;
            Ok(result
                .into_iter()
                .map(|r| Row {
                    columns: r.columns,
                    values: r.values,
                })
                .collect())
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Stub for native testing
            let _ = (sql, params);
            Ok(vec![])
        }
    }

    /// Execute a SQL statement that modifies data.
    ///
    /// # Errors
    /// Returns error string if execution fails.
    pub fn execute(sql: &str, params: &[String]) -> Result<u32, String> {
        #[cfg(target_arch = "wasm32")]
        {
            use crate::brio_host::sql_state as wit;
            wit::execute(sql, params)
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Stub for native testing
            let _ = (sql, params);
            Ok(0)
        }
    }
}

/// Service Mesh interface bindings.
pub mod service_mesh {
    /// Payload variant for mesh calls.
    #[derive(Debug, Clone)]
    pub enum Payload {
        /// JSON-encoded payload data.
        Json(String),
        /// Binary payload data.
        Binary(Vec<u8>),
    }

    /// Call a component via the service mesh.
    ///
    /// # Errors
    /// Returns error string if call fails.
    pub fn call(target: &str, method: &str, args: Payload) -> Result<Payload, String> {
        #[cfg(target_arch = "wasm32")]
        {
            use crate::brio_host::service_mesh as wit;

            let wit_args = match args {
                Payload::Json(s) => wit::Payload::Json(s),
                Payload::Binary(b) => wit::Payload::Binary(b),
            };

            let result = wit::call(target, method, wit_args)?;

            Ok(match result {
                wit::Payload::Json(s) => Payload::Json(s),
                wit::Payload::Binary(b) => Payload::Binary(b),
            })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Stub for native testing
            let _ = (target, method, args);
            Ok(Payload::Json(r#"{"status":"accepted"}"#.to_string()))
        }
    }
}

/// Brio Core bindings.
pub mod brio {
    /// Core Brio services and interfaces.
    pub mod core {
        /// Task planning and decomposition services.
        pub mod planner {
            /// Represents a sub-step of a larger task.
            #[derive(Debug, Clone)]
            pub struct Subtask {
                /// Unique identifier for this subtask.
                pub id: String,
                /// Description of what this subtask should accomplish.
                pub description: String,
            }

            /// A sequence of steps to achieve an objective.
            #[derive(Debug, Clone)]
            pub struct Plan {
                /// Ordered list of subtasks to execute.
                pub steps: Vec<Subtask>,
            }

            /// Decomposes a high-level objective into actionable subtasks.
            ///
            /// # Errors
            /// Returns an error string if decomposition fails.
            pub fn decompose(objective: &str) -> Result<Plan, String> {
                #[cfg(target_arch = "wasm32")]
                {
                    use crate::brio_host::brio::core::planner as wit;
                    let result = wit::decompose(objective)?;

                    // Map WIT types to our facade types
                    Ok(Plan {
                        steps: result
                            .steps
                            .into_iter()
                            .map(|s| Subtask {
                                id: s.id,
                                description: s.description,
                            })
                            .collect(),
                    })
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    // Stub for native testing
                    let _ = objective;
                    Ok(Plan {
                        steps: vec![Subtask {
                            id: "step1".to_string(),
                            description: "Mock step 1".to_string(),
                        }],
                    })
                }
            }
        }
    }
}
