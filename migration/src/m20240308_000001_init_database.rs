use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240308_000001_init_database"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    #[allow(clippy::too_many_lines)]
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Channel::Table)
                    .col(
                        ColumnDef::new(Channel::Id)
                            .primary_key()
                            .not_null()
                            .big_unsigned(),
                    )
                    .col(ColumnDef::new(Channel::Guild).not_null().big_unsigned())
                    .col(ColumnDef::new(Channel::Message).big_unsigned().unique_key())
                    .col(ColumnDef::new(Channel::NotifiedRole).big_unsigned())
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Game::Table)
                    .to_owned()
                    .col(ColumnDef::new(Game::Id).not_null().big_unsigned())
                    .col(ColumnDef::new(Game::Channel).not_null().big_unsigned())
                    .primary_key(Index::create().col(Game::Id).col(Game::Channel))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-game-channel")
                            .from(Game::Table, Game::Channel)
                            .to(Channel::Table, Channel::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Target::Table)
                    .to_owned()
                    .col(ColumnDef::new(Target::Id).not_null().big_unsigned())
                    .col(ColumnDef::new(Target::Channel).not_null().big_unsigned())
                    .primary_key(Index::create().col(Target::Id).col(Target::Channel))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-target-channel")
                            .from(Target::Table, Target::Channel)
                            .to(Channel::Table, Channel::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Target::Table)
                    .col(Target::Channel)
                    .name("idx-target-channel")
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Game::Table)
                    .col(Game::Channel)
                    .name("idx-game-channel")
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Channel::Table)
                    .col(Channel::Guild)
                    .name("idx-channel-guild")
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Channel::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Game::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Target::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Channel {
    Table,
    Id,
    Guild,
    Message,
    NotifiedRole,
}
#[derive(Iden)]
pub enum Game {
    Table,
    Id,
    Channel,
}
#[derive(Iden)]
pub enum Target {
    Table,
    Id,
    Channel,
}
