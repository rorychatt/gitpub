use gitpub_core::Database;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;

pub struct TestDatabase {
    _container: ContainerAsync<Postgres>,
    pub db: Database,
}

impl TestDatabase {
    pub async fn new() -> Self {
        let container = Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");
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
