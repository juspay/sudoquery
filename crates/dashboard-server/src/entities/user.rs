use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique_indexed)]
    pub keycloak_user_id: String,
    #[sea_orm(unique_indexed)]
    pub email: String,
    pub username: String,
    pub deleted_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::organization_membership::Entity")]
    OrganizationMembership,
    #[sea_orm(has_many = "super::project_membership::Entity")]
    ProjectMembership,
    #[sea_orm(has_many = "super::invitation::Entity")]
    Invitation,
}

impl Related<super::organization_membership::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrganizationMembership.def()
    }
}

impl Related<super::project_membership::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectMembership.def()
    }
}

impl Related<super::invitation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invitation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    /// Find only active (non-deleted) users
    pub fn find_active() -> Select<Self> {
        Self::find().filter(Column::DeletedAt.is_null())
    }
}
