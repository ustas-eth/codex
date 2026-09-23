use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

/// Configured cyber treatment. `auto` omits the request field, even when a
/// parent turn used an explicit program. Authorization remains server-owned.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CyberAccessProgramPreference {
    Auto,
    Standard,
    DaybreakBlue,
    DaybreakRed,
}
