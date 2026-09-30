use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(EnumIter, DeriveActiveEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "invitation_type")]
pub enum InvitationType {
    #[sea_orm(string_value = "organization")]
    Organization,
    #[sea_orm(string_value = "project")]
    Project,
}

#[derive(EnumIter, DeriveActiveEnum, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "invitation_status")]
pub enum InvitationStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "accepted")]
    Accepted,
    #[sea_orm(string_value = "revoked")]
    Revoked,
    #[sea_orm(string_value = "expired")]
    Expired,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "invitations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub email: String,
    pub invitation_type: InvitationType,
    pub target_id: Uuid,
    pub role: String,
    pub invited_by: Uuid,
    pub status: InvitationStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub expires_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::InvitedBy",
        to = "super::user::Column::Id"
    )]
    Inviter,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Inviter.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    /// Find pending invitations
    pub fn find_pending() -> Select<Self> {
        Self::find().filter(Column::Status.eq(InvitationStatus::Pending))
    }
}
