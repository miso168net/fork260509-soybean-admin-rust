//! F030 US2 gender-display-filter: sys_user.gender 欄位 + PG enum type 建立 + seed
//! up(): 1) Postgres: CREATE TYPE gender AS ENUM('male','female')
//!        2) ALTER TABLE sys_user ADD COLUMN gender gender NULL
//!        3) UPDATE sys_user SET gender (Soybean/Administrator→male, GeneralUser→female)
//! down(): DROP COLUMN gender, DROP TYPE gender (Postgres only)

use sea_orm::EnumIter;
use sea_orm_migration::{
    prelude::{sea_query::extension::postgres::Type, *},
    sea_orm::{ConnectionTrait, DbBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Step 1: Create the gender PG enum type (Postgres only)
        if db.get_database_backend() == DbBackend::Postgres {
            manager
                .create_type(
                    Type::create()
                        .as_enum(Alias::new("gender"))
                        .values([GenderEnum::Male, GenderEnum::Female])
                        .to_owned(),
                )
                .await?;
        }

        // Step 2: ALTER TABLE sys_user ADD COLUMN gender gender NULL
        let alter_stmt = Statement::from_string(
            db.get_database_backend(),
            match db.get_database_backend() {
                DbBackend::Postgres => {
                    "ALTER TABLE sys_user ADD COLUMN gender gender NULL".to_string()
                }
                DbBackend::MySql => {
                    "ALTER TABLE sys_user ADD COLUMN gender ENUM('male','female') NULL".to_string()
                }
                DbBackend::Sqlite => {
                    "ALTER TABLE sys_user ADD COLUMN gender TEXT NULL".to_string()
                }
            },
        );
        db.execute(alter_stmt).await?;

        // Step 3: Seed gender values (two separate execute() calls — PG rejects multiple
        // statements in one prepared-query call)
        let update_male = Statement::from_string(
            db.get_database_backend(),
            match db.get_database_backend() {
                DbBackend::Postgres => {
                    "UPDATE sys_user SET gender = 'male'::gender WHERE username IN ('Soybean', 'Administrator')".to_string()
                }
                _ => {
                    "UPDATE sys_user SET gender = 'male' WHERE username IN ('Soybean', 'Administrator')".to_string()
                }
            },
        );
        db.execute(update_male).await?;

        let update_female = Statement::from_string(
            db.get_database_backend(),
            match db.get_database_backend() {
                DbBackend::Postgres => {
                    "UPDATE sys_user SET gender = 'female'::gender WHERE username = 'GeneralUser'".to_string()
                }
                _ => {
                    "UPDATE sys_user SET gender = 'female' WHERE username = 'GeneralUser'".to_string()
                }
            },
        );
        db.execute(update_female).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Drop the gender column
        let drop_col_stmt = Statement::from_string(
            db.get_database_backend(),
            "ALTER TABLE sys_user DROP COLUMN gender".to_string(),
        );
        db.execute(drop_col_stmt).await?;

        // Drop the gender type (Postgres only)
        if db.get_database_backend() == DbBackend::Postgres {
            manager
                .drop_type(Type::drop().name(Alias::new("gender")).to_owned())
                .await?;
        }

        Ok(())
    }
}

#[derive(DeriveIden, EnumIter)]
pub enum GenderEnum {
    #[sea_orm(iden = "gender")]
    Enum,
    #[sea_orm(iden = "male")]
    Male,
    #[sea_orm(iden = "female")]
    Female,
}
