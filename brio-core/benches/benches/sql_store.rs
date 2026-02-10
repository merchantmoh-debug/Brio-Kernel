//! Benchmarks for SQL query execution in store/impl.rs
//!
//! Performance-critical paths:
//! - `SqlStore::query`: SELECT operations with policy enforcement
//! - `SqlStore::execute`: INSERT/UPDATE/DELETE operations
//! - `convert_cell`: Type conversion from database to string

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

/// Simulated cell conversion - mirrors store/impl.rs logic
fn convert_cell(type_info: &str, value: &str) -> String {
    if value == "NULL" || type_info == "null" {
        return "NULL".to_string();
    }

    match type_info {
        "TEXT" | "VARCHAR" | "STRING" => value.to_string(),
        "INTEGER" | "INT" | "BIGINT" => {
            if let Ok(n) = value.parse::<i64>() {
                n.to_string()
            } else {
                "UNSUPPORTED_TYPE".to_string()
            }
        }
        "REAL" | "FLOAT" | "DOUBLE" => {
            if let Ok(f) = value.parse::<f64>() {
                f.to_string()
            } else {
                "UNSUPPORTED_TYPE".to_string()
            }
        }
        "BLOB" => format!("<BLOB:{}>", value.len()),
        _ => "UNSUPPORTED_TYPE".to_string(),
    }
}

/// Simulated policy check
fn authorize_policy(_scope: &str, sql: &str, allowed_tables: &[&str]) -> bool {
    // Check if SQL references only allowed tables
    for table in allowed_tables {
        if sql
            .to_lowercase()
            .contains(&format!("from {table}").to_lowercase())
        {
            return true;
        }
    }
    // Allow simple patterns
    sql.to_lowercase().starts_with("select")
        && !sql.to_lowercase().contains("drop")
        && !sql.to_lowercase().contains("delete")
}

/// Simulated query parameter binding
fn bind_params(sql: &str, params: &[String]) -> String {
    let mut result = sql.to_string();
    for param in params {
        result = result.replacen('?', &format!("'{}'", param.replace('\'', "''")), 1);
    }
    result
}

fn bench_convert_cell(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_store/convert_cell");

    let test_cases = [
        ("null", "", "NULL"),
        ("TEXT", "Hello, World!", "Hello, World!"),
        ("INTEGER", "42", "42"),
        ("BIGINT", "9223372036854775807", "9223372036854775807"),
        ("REAL", "3.14159", "3.14159"),
        ("BLOB", "binary_data_here", "<BLOB:16>"),
        ("UNKNOWN", "something", "UNSUPPORTED_TYPE"),
    ];

    for (type_info, value, _expected) in &test_cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(type_info.to_string()),
            &(type_info, value),
            |b, (ti, val)| b.iter(|| convert_cell(black_box(ti), black_box(val))),
        );
    }

    group.finish();
}

fn bench_convert_cell_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_store/convert_cell_batch");

    // Simulate converting a row of mixed types
    let row_types = ["INTEGER", "TEXT", "REAL", "TEXT", "INTEGER"];
    let row_values = ["123", "test string", "45.67", "another text", "789"];

    group.bench_function("5_columns", |b| {
        b.iter(|| {
            let mut result = Vec::with_capacity(5);
            for (i, type_info) in row_types.iter().enumerate() {
                result.push(convert_cell(black_box(type_info), black_box(row_values[i])));
            }
            black_box(result)
        });
    });

    // Simulate converting many rows
    let batch_size = 100;
    let rows: Vec<(&str, &str)> = (0..batch_size)
        .map(|i| {
            let types = ["INTEGER", "TEXT", "REAL"];
            let type_info = types[i % 3];
            let value = format!("value_{i}");
            (type_info, value.leak() as &'static str)
        })
        .collect();

    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{batch_size}_rows_3_cols")),
        &batch_size,
        |b, _| {
            b.iter(|| {
                let mut results = Vec::with_capacity(batch_size);
                for (type_info, value) in &rows {
                    results.push(convert_cell(black_box(type_info), black_box(*value)));
                }
                black_box(results)
            });
        },
    );

    group.finish();
}

fn bench_authorize_policy(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_store/authorize_policy");

    let queries = [
        ("simple_select", "SELECT * FROM users WHERE id = 1"),
        (
            "complex_select",
            "SELECT u.id, u.name, o.total FROM users u JOIN orders o ON u.id = o.user_id WHERE o.total > 100",
        ),
        (
            "select_with_params",
            "SELECT * FROM products WHERE category = ? AND price < ?",
        ),
        ("rejected_drop", "DROP TABLE users"),
        ("rejected_delete", "DELETE FROM users WHERE id = 1"),
    ];

    let allowed_tables = &["users", "orders", "products"];

    for (name, query) in &queries {
        group.bench_with_input(BenchmarkId::from_parameter(*name), query, |b, q| {
            b.iter(|| {
                authorize_policy(
                    black_box("test_scope"),
                    black_box(q),
                    black_box(allowed_tables),
                )
            });
        });
    }

    group.finish();
}

fn bench_bind_params(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_store/bind_params");

    let test_cases = [
        ("1_param", "SELECT * FROM users WHERE id = ?", vec!["123"]),
        (
            "2_params",
            "SELECT * FROM users WHERE id = ? AND name = ?",
            vec!["123", "John"],
        ),
        (
            "5_params",
            "INSERT INTO users VALUES (?, ?, ?, ?, ?)",
            vec!["1", "John", "Doe", "john@example.com", "active"],
        ),
        (
            "special_chars",
            "SELECT * FROM users WHERE name = ?",
            vec!["O'Brien"],
        ),
    ];

    for (name, sql, params) in &test_cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(*name),
            &(sql, params.clone()),
            |b, (sql, params)| {
                let params_owned: Vec<String> = params.iter().map(std::string::ToString::to_string).collect();
                b.iter(|| bind_params(black_box(sql), black_box(&params_owned)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_convert_cell,
    bench_convert_cell_batch,
    bench_authorize_policy,
    bench_bind_params
);
criterion_main!(benches);
