pub use sea_orm_migration::prelude::*;

mod m20240308_000001_init_database;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20240308_000001_init_database::Migration)]
    }
}
