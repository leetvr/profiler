use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSummary {
    pub name: String,
    pub value: f32,
    pub last_value: f32,
    pub average_value: f32,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileRun {
    pub description: String,
    pub timestamp: u64, // epoch
    pub result: bool,
    pub metrics: Vec<MetricSummary>,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileSummary {
    pub description: String,
    pub timestamp: u64, // epoch
    pub result: bool,
    pub id: usize,
}
