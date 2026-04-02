use crate::state::types::ResourceType;

#[derive(Debug)]
pub enum Action {
    Create(ResourcePlan),
    Update(ResourcePlan),
    Delete(ResourcePlan),
    NoOp(String),
}

#[derive(Debug)]
pub struct ResourcePlan {
    pub resource_key: String,
    pub resource_type: ResourceType,
    pub topic_id: Option<i32>,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug)]
pub struct FieldChange {
    pub field: String,
    pub old: Option<String>,
    pub new: Option<String>,
}
