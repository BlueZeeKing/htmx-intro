use serde::Serialize;
use sqlx::FromRow;

pub mod routes;

#[derive(Serialize, FromRow)]
struct Task<'a> {
    name: &'a str,
    completed: bool,
}
