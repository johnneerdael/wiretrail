use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RawDoc {
    pub log: RawLog,
}

#[derive(Debug, Deserialize)]
pub struct RawLog {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub creator: RawCreator,
    #[serde(default)]
    pub browser: Option<RawCreator>,
    #[serde(default)]
    pub entries: Vec<RawEntry>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawCreator {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct RawEntry {
    #[serde(rename = "startedDateTime", default)]
    pub started_date_time: String,
    #[serde(default)]
    pub time: f64,
    pub request: RawRequest,
    pub response: RawResponse,
    #[serde(default)]
    pub timings: RawTimings,
    #[serde(rename = "serverIPAddress", default)]
    pub server_ip_address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "httpVersion", default)]
    pub http_version: String,
    #[serde(default)]
    pub headers: Vec<RawNameValue>,
    #[serde(rename = "queryString", default)]
    pub query_string: Vec<RawNameValue>,
    #[serde(rename = "postData", default)]
    pub post_data: Option<RawPostData>,
    #[serde(rename = "bodySize", default)]
    pub body_size: i64,
}

#[derive(Debug, Deserialize)]
pub struct RawResponse {
    #[serde(default)]
    pub status: i64,
    #[serde(rename = "statusText", default)]
    pub status_text: String,
    #[serde(default)]
    pub headers: Vec<RawNameValue>,
    #[serde(default)]
    pub content: RawContent,
    #[serde(rename = "redirectURL", default)]
    pub redirect_url: Option<String>,
    #[serde(rename = "headersSize", default)]
    pub headers_size: i64,
    #[serde(rename = "bodySize", default)]
    pub body_size: i64,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawContent {
    #[serde(default)]
    pub size: i64,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawPostData {
    #[serde(rename = "mimeType", default)]
    pub mime_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawTimings {
    #[serde(default)]
    pub blocked: Option<f64>,
    #[serde(default)]
    pub dns: Option<f64>,
    #[serde(default)]
    pub connect: Option<f64>,
    #[serde(default)]
    pub send: f64,
    #[serde(default)]
    pub wait: f64,
    #[serde(default)]
    pub receive: f64,
    #[serde(default)]
    pub ssl: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct RawNameValue {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}
