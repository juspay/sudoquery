use canonical_event::OrgId;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    // `String` rather than `ProjectId`: sea-orm's `PrimaryKeyTrait` requires
    // `TryFromU64`/`FromValueTuple`, which the canonical-event newtypes do
    // not (and cannot, per orphan rules) implement. The sqlx `db` layer is
    // the authoritative typed boundary for slugs.
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub organization_id: Option<OrgId>,
    pub name: String,
    pub timezone: Option<String>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organization::Entity",
        from = "Column::OrganizationId",
        to = "super::organization::Column::Id"
    )]
    Organization,
    #[sea_orm(has_many = "super::project_membership::Entity")]
    ProjectMembership,
    #[sea_orm(has_many = "super::project_token::Entity")]
    ProjectToken,
}

impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl Related<super::project_membership::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectMembership.def()
    }
}

impl Related<super::project_token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProjectToken.def()
    }
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        super::project_membership::Relation::User.def()
    }
    fn via() -> Option<RelationDef> {
        Some(super::project_membership::Relation::Project.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    /// Find only active (non-deleted) projects
    pub fn find_active() -> Select<Self> {
        Self::find().filter(Column::DeletedAt.is_null())
    }
}
