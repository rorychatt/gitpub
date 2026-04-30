use gitpub_core::Database;
use testcontainers::{clients::Cli, Container};
use testcontainers_modules::postgres::Postgres;

pub struct TestDatabase<'a> {
    _container: Container<'a, Postgres>,
    pub db: Database,
}

impl<'a> TestDatabase<'a> {
    pub async fn new(docker: &'a Cli) -> Self {
        let container = docker.run(Postgres::default());
        let port = container.get_host_port_ipv4(5432);
        let db_url = format!("postgresql://postgres:postgres@localhost:{}/postgres", port);

        let db = Database::new(&db_url)
            .await
            .expect("Failed to create test database");

        Self {
            _container: container,
            db,
        }
    }
}
