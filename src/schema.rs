use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefecture {
    pub pref_code: u8,
    pub pref_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct City {
    pub pref_code: u8,
    pub city_code: String,
    pub city_name: String,
    pub big_city_flag: String,
}
