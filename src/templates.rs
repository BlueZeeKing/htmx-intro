use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct Tasks {
    pub tasks: Vec<crate::Task>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct Login {}

#[derive(Template)]
#[template(path = "partials/task.html")]
pub struct Task {
    pub task: crate::Task,
}

#[derive(Template)]
#[template(path = "partials/list.html")]
pub struct List {
    pub tasks: Vec<crate::Task>,
}
