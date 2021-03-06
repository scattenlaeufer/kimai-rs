use chrono::prelude::*;
use clap::crate_name;
use prettytable::{cell, format, row, Table};
use reqwest::header::{self, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M";
pub const TIME_FORMAT: &str = "%H:%M";

trait QueryValue {
    fn process(&self) -> String;
}

impl QueryValue for Vec<usize> {
    fn process(&self) -> String {
        self.iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }
}

impl QueryValue for &str {
    fn process(&self) -> String {
        self.to_string()
    }
}

impl QueryValue for String {
    fn process(&self) -> String {
        self.to_string()
    }
}

impl QueryValue for usize {
    fn process(&self) -> String {
        self.to_string()
    }
}

impl QueryValue for DateTime<Local> {
    fn process(&self) -> String {
        self.naive_local().to_string()
    }
}

macro_rules! query{
    ($(($key:expr, $value:expr)),*) => {{
        let mut queries = HashMap::new();
        $(if let Some(v) = $value {
            queries.insert($key, QueryValue::process(&v));
        };)*
        if queries.is_empty() {
            None
        } else {
            Some(queries)
        }
    }}
}

#[derive(Debug)]
pub enum KimaiError {
    XdgBaseDirectories(String),
    IO(String),
    Toml(String),
    Utf8(String),
    Reqwest(String),
    ChronoParse(String),
    Config(String),
    Api(String),
    Other(String),
}

impl std::error::Error for KimaiError {}

impl fmt::Display for KimaiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KimaiError::XdgBaseDirectories(e) => write!(f, "XDG BaseDirectories Error: {}", e),
            KimaiError::IO(e) => write!(f, "IO Error: {}", e),
            KimaiError::Toml(e) => write!(f, "TOML Error: {}", e),
            KimaiError::Utf8(e) => write!(f, "UTF-8 Error: {}", e),
            KimaiError::Reqwest(e) => write!(f, "Reqwest Error: {}", e),
            KimaiError::ChronoParse(e) => write!(f, "Chrono Parser Error: {}", e),
            KimaiError::Config(e) => write!(f, "Config Error: {}", e),
            KimaiError::Api(e) => write!(f, "API Error: {}", e),
            KimaiError::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl From<xdg::BaseDirectoriesError> for KimaiError {
    fn from(error: xdg::BaseDirectoriesError) -> KimaiError {
        KimaiError::XdgBaseDirectories(error.to_string())
    }
}

impl From<std::io::Error> for KimaiError {
    fn from(error: std::io::Error) -> KimaiError {
        KimaiError::IO(error.to_string())
    }
}

impl From<toml::de::Error> for KimaiError {
    fn from(error: toml::de::Error) -> KimaiError {
        KimaiError::Toml(error.to_string())
    }
}

impl From<std::str::Utf8Error> for KimaiError {
    fn from(error: std::str::Utf8Error) -> KimaiError {
        KimaiError::Utf8(error.to_string())
    }
}

impl From<reqwest::Error> for KimaiError {
    fn from(error: reqwest::Error) -> KimaiError {
        KimaiError::Reqwest(error.to_string())
    }
}

impl From<chrono::format::ParseError> for KimaiError {
    fn from(error: chrono::format::ParseError) -> KimaiError {
        KimaiError::ChronoParse(error.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    host: String,
    user: String,
    password: Option<String>,
    pass_path: Option<String>,
}

#[derive(Debug)]
pub struct Config {
    host: String,
    user: String,
    password: String,
}

impl Config {
    pub fn new(host: String, user: String, password: String) -> Self {
        Config {
            host,
            user,
            password,
        }
    }
    pub fn from_path(path: &Path) -> Result<Self, KimaiError> {
        let config_string = fs::read_to_string(path)?;
        let config_file = toml::from_str::<ConfigFile>(&config_string)?;
        if let Some(p) = config_file.password {
            Ok(Config {
                host: config_file.host,
                user: config_file.user,
                password: p,
            })
        } else if let Some(p) = config_file.pass_path {
            let pass_cmd = Command::new("pass").arg(p).output()?;
            Ok(Config {
                host: config_file.host,
                user: config_file.user,
                password: std::str::from_utf8(&pass_cmd.stdout)?.trim().into(),
            })
        } else {
            Err(KimaiError::Config(
                "No password give in config!".to_string(),
            ))
        }
    }

    pub fn from_xdg() -> Result<Self, KimaiError> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix(crate_name!())?;
        let config_path = xdg_dirs
            .find_config_file("config.toml")
            .ok_or_else(|| KimaiError::Config("config file not found!".to_string()))?;
        Self::from_path(Path::new(&config_path))
    }
}

#[derive(Debug, Deserialize)]
pub struct User {
    id: usize,
    username: String,
    enabled: bool,
    roles: Vec<String>,
    language: String,
    timezone: String,
    alias: Option<String>,
    title: Option<String>,
    avatar: Option<String>,
    teams: Vec<Team>,
}

#[derive(Debug, Deserialize)]
pub struct Team {
    id: usize,
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Customer {
    id: usize,
    name: String,
    visible: bool,
    color: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    id: usize,
    name: String,
    customer: usize,
    parent_title: String,
    visible: bool,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShortProject {
    id: usize,
    name: String,
    visible: bool,
    color: Option<String>,
    customer: Customer,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    id: usize,
    name: String,
    project: Option<usize>,
    parent_title: Option<String>,
    visible: bool,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShortActivity {
    id: usize,
    name: String,
    visible: bool,
    color: Option<String>,
    project: Option<ShortProject>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TimesheetRecord {
    pub id: usize,
    description: Option<String>,
    begin: DateTime<Local>,
    end: Option<DateTime<Local>>,
    duration: i64,
    project: usize,
    activity: usize,
    user: usize,
    tags: Vec<String>,
}

impl TimesheetRecord {
    pub fn print_table(&self) {
        let description = match &self.description {
            Some(d) => d,
            None => "",
        };
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(row!["Attribute", "Value"]);
        table.add_row(row!["ID", self.id]);
        // TODO: resolve project, activity and user IDs to the actual names
        table.add_row(row!["Project", self.project]);
        table.add_row(row!["Activity", self.activity]);
        table.add_row(row!["User", self.user]);
        table.add_row(row!["Begin", self.begin]);
        if let Some(end) = self.end {
            table.add_row(row!["End", end]);
        }
        if self.duration != 0 {
            let d = chrono::Duration::seconds(self.duration);
            table.add_row(row![
                "Duration",
                format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60)
            ]);
        }
        table.add_row(row!["Description", description]);
        table.add_row(row!["Tags", self.tags.join(", ")]);
        table.printstd();
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewTimesheetRecord {
    project: usize,
    activity: usize,
    begin: NaiveDateTime,
    end: Option<NaiveDateTime>,
    description: Option<String>,
    //user: usize,
    tags: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimesheetRecordEntity {
    id: usize,
    begin: DateTime<Local>,
    end: Option<DateTime<Local>>,
    duration: i64,
    description: Option<String>,
    rate: f32,
    internal_rate: f32,
    #[serde(default)]
    billable: bool,
    project: ShortProject,
    activity: ShortActivity,
}

fn get_headers(config: &Config) -> Result<header::HeaderMap, KimaiError> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-auth-user"),
        HeaderValue::from_str(&config.user).unwrap(),
    );
    headers.insert(
        HeaderName::from_static("x-auth-token"),
        HeaderValue::from_str(&config.password).unwrap(),
    );
    Ok(headers)
}

async fn check_response(response: reqwest::Response) -> Result<reqwest::Response, KimaiError> {
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(KimaiError::Api(response.text().await?))
    }
}

async fn make_get_request<T>(
    config: &Config,
    api_endpoint: &str,
    parameters: Option<HashMap<&str, String>>,
) -> Result<T, KimaiError>
where
    T: for<'de> Deserialize<'de>,
{
    let url = format!("{}/{}", config.host, api_endpoint);
    let mut request_builder = reqwest::Client::builder()
        .default_headers(get_headers(config)?)
        .build()?
        .get(&url);
    if let Some(p) = parameters {
        request_builder = request_builder.query(&p);
    }
    Ok(check_response(request_builder.send().await?)
        .await?
        .json()
        .await?)
}

async fn make_post_request<T, V>(
    config: &Config,
    api_endpoint: &str,
    body: T,
    parameters: Option<HashMap<&str, String>>,
) -> Result<V, KimaiError>
where
    T: Serialize,
    V: for<'de> Deserialize<'de>,
{
    let url = format!("{}/{}", config.host, api_endpoint);
    let mut request_builder = reqwest::Client::builder()
        .default_headers(get_headers(config)?)
        .build()?
        .post(&url)
        .json(&body);
    if let Some(p) = parameters {
        request_builder = request_builder.query(&p);
    }
    Ok(check_response(request_builder.send().await?)
        .await?
        .json()
        .await?)
}
async fn make_patch_request<T, V>(
    config: &Config,
    api_endpoint: &str,
    body: Option<T>,
    parameters: Option<HashMap<&str, String>>,
) -> Result<V, KimaiError>
where
    T: Serialize,
    V: for<'de> Deserialize<'de>,
{
    let url = format!("{}/{}", config.host, api_endpoint);
    let mut request_builder = reqwest::Client::builder()
        .default_headers(get_headers(config)?)
        .build()?
        .patch(&url);
    if let Some(b) = body {
        request_builder = request_builder.json(&b);
    }
    if let Some(p) = parameters {
        request_builder = request_builder.query(&p);
    }
    Ok(check_response(request_builder.send().await?)
        .await?
        .json()
        .await?)
}

/// Load a configuration file.
///
/// If `config_path` is `None`, it get's loaded from the XDG configuration
/// folder.
pub fn load_config(config_path: Option<String>) -> Result<Config, KimaiError> {
    match config_path {
        Some(p) => Config::from_path(Path::new(&p)),
        None => Config::from_xdg(),
    }
}

/// Get all available customers
pub async fn get_customers(
    config: &Config,
    term: Option<String>,
) -> Result<Vec<Customer>, KimaiError> {
    make_get_request(config, "api/customers", query!(("term", term))).await
}

/// Get all available projects
pub async fn get_projects(
    config: &Config,
    customers: Option<Vec<usize>>,
    term: Option<String>,
) -> Result<Vec<Project>, KimaiError> {
    make_get_request(
        config,
        "api/projects",
        query!(("customers", customers), ("term", term)),
    )
    .await
}

/// Get all available activities
pub async fn get_activities(
    config: &Config,
    projects: Option<Vec<usize>>,
    term: Option<String>,
) -> Result<Vec<Activity>, KimaiError> {
    make_get_request(
        config,
        "api/activities",
        query!(("projects", projects), ("term", term)),
    )
    .await
}

/// Get a timesheet with all it's records
pub async fn get_timesheet(
    config: &Config,
    user: Option<usize>,
    customers: Option<Vec<usize>>,
    projects: Option<Vec<usize>>,
    activities: Option<Vec<usize>>,
) -> Result<Vec<TimesheetRecord>, KimaiError> {
    // TODO: Implemnt this to get the entire timesheet records
    make_get_request(
        config,
        "api/timesheets",
        query!(
            ("user", user),
            ("customers", customers),
            ("projects", projects),
            ("activities", activities)
        ),
    )
    .await
}

/// Begin a new timesheet record. If no begin time is given, the current time
/// is used.
pub async fn begin_timesheet_record(
    config: &Config,
    // TODO: find out why adding a user doesn't work
    _user: usize,
    project: usize,
    activity: usize,
    begin: DateTime<Local>,
    description: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<TimesheetRecord, KimaiError> {
    let record = NewTimesheetRecord {
        project,
        activity,
        begin: begin.naive_local(),
        end: None,
        description,
        tags: tags.map(|t| t.join(",")),
    };
    make_post_request(config, "api/timesheets", record, None).await
}

/// End a given timesheet record. The current time is set as end time.
pub async fn end_timesheet_record(
    config: &Config,
    id: usize,
) -> Result<TimesheetRecord, KimaiError> {
    make_patch_request::<Vec<String>, TimesheetRecord>(
        config,
        &format!("api/timesheets/{}/stop", id),
        None,
        None,
    )
    .await
}

/// Get data of the user that is making logging in to make the request.
pub async fn get_current_user(config: &Config) -> Result<User, KimaiError> {
    make_get_request(config, "api/users/me", None).await
}

/// Log an entire timesheet record. If no end time is given, the current time
/// is used.
#[allow(clippy::too_many_arguments)]
pub async fn log_timesheet_record(
    config: &Config,
    // TODO: find out why adding a user doesn't work
    _user: usize,
    project: usize,
    activity: usize,
    begin: DateTime<Local>,
    end: Option<DateTime<Local>>,
    description: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<TimesheetRecord, KimaiError> {
    let record = NewTimesheetRecord {
        project,
        activity,
        begin: begin.naive_local(),
        end: end.map(|e| e.naive_local()),
        description,
        tags: tags.map(|t| t.join(",")),
    };
    make_post_request(config, "api/timesheets", record, None).await
}

/// Get all currently active timesheet records
pub async fn get_active_timesheet(
    config: &Config,
) -> Result<Vec<TimesheetRecordEntity>, KimaiError> {
    make_get_request(&config, "api/timesheets/active", None).await
}

/// Get recent timesheet records
pub async fn get_recent_timesheet(
    config: &Config,
    user: Option<usize>,
    begin: Option<DateTime<Local>>,
) -> Result<Vec<TimesheetRecordEntity>, KimaiError> {
    make_get_request(
        &config,
        "api/timesheets/recent",
        query!(("user", user), ("begin", begin)),
    )
    .await
}

/// Get the data of one given timesheet record
pub async fn get_timesheet_record(
    config: &Config,
    id: usize,
) -> Result<TimesheetRecord, KimaiError> {
    make_get_request(&config, &format!("api/timesheets/{}", id), None).await
}

#[tokio::main]
pub async fn print_customers(
    config_path: Option<String>,
    term: Option<String>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;
    let customers = get_customers(&config, term).await?;

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(row!["ID", "Name"]);
    for customer in customers {
        table.add_row(row![customer.id, customer.name]);
    }

    table.printstd();

    Ok(())
}

#[tokio::main]
pub async fn print_projects(
    config_path: Option<String>,
    customers: Option<Vec<usize>>,
    term: Option<String>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;
    let projects = get_projects(&config, customers, term).await?;

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(row!["ID", "Name", "Customer ID", "Customer Name"]);
    for project in projects {
        table.add_row(row![
            r->project.id,
            project.name,
            r->project.customer,
            project.parent_title
        ]);
    }

    table.printstd();

    Ok(())
}

#[tokio::main]
pub async fn print_activities(
    config_path: Option<String>,
    projects: Option<Vec<usize>>,
    term: Option<String>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;
    let activities = get_activities(&config, projects, term).await?;

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(row!["ID", "Name", "Project ID", "Project Name"]);
    for activity in activities {
        let project_str = match activity.project {
            Some(p) => p.to_string(),
            None => "".to_string(),
        };
        table.add_row(row![
            r->activity.id,
            activity.name,
            r->project_str,
            activity.parent_title.unwrap_or_default()
        ]);
    }

    table.printstd();

    Ok(())
}

fn print_timesheets(records: &[TimesheetRecord]) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(row![
        "ID",
        "Begin",
        "End",
        "Duration",
        "Project",
        "Activity",
        "Description"
    ]);
    for record in records {
        let description = match &record.description {
            Some(d) => d.to_string(),
            None => "".into(),
        };
        let end = match record.end {
            Some(e) => e.format("%Y-%m-%d %H:%M").to_string(),
            None => "".to_string(),
        };
        let d = chrono::Duration::seconds(record.duration);
        let d_str = format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60);
        table.add_row(row![
            r->record.id,
            record.begin.format("%Y-%m-%d %H:%M"),
            end,
            r->d_str,
            r->record.project,
            r->record.activity,
            description,
        ]);
    }

    table.printstd();
}

fn print_timesheet_entities(records: &[TimesheetRecordEntity]) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.set_titles(row![
        "ID",
        "Begin",
        "End",
        "Duration",
        "Project",
        "Activity",
        "Description"
    ]);
    for record in records {
        let description = match &record.description {
            Some(d) => d.to_string(),
            None => "".into(),
        };
        let end = match record.end {
            Some(e) => e.format("%Y-%m-%d %H:%M").to_string(),
            None => "".to_string(),
        };
        let d = chrono::Duration::seconds(record.duration);
        let d_str = format!("{}:{:02}", d.num_hours(), d.num_minutes() % 60);
        table.add_row(row![
            r->record.id,
            record.begin.format("%Y-%m-%d %H:%M"),
            end,
            r->d_str,
            format!("{} ({})", record.project.id, record.project.name),
            format!("{} ({})", record.activity.id, record.activity.name),
            description,
        ]);
    }

    table.printstd();
}

#[tokio::main]
pub async fn print_timesheet(
    config_path: Option<String>,
    user: Option<usize>,
    customers: Option<Vec<usize>>,
    projects: Option<Vec<usize>>,
    activities: Option<Vec<usize>>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;
    let timesheet_records = get_timesheet(&config, user, customers, projects, activities).await?;

    print_timesheets(&timesheet_records);

    Ok(())
}

fn str_to_datetime(date_str: &str) -> Result<DateTime<Local>, KimaiError> {
    match NaiveDateTime::parse_from_str(date_str, DATETIME_FORMAT) {
        Ok(d) => Ok(Local.from_local_datetime(&d).unwrap()),
        Err(_) => match NaiveTime::parse_from_str(date_str, TIME_FORMAT) {
            Ok(t) => Ok(Local::today().and_time(t).unwrap()),
            Err(e) => Err(KimaiError::from(e)),
        },
    }
}

fn get_datetime(datetime_str: Option<String>) -> Result<DateTime<Local>, KimaiError> {
    match datetime_str {
        Some(s) => str_to_datetime(&s),
        None => {
            let mut now = Local::now();
            now = now - chrono::Duration::nanoseconds(now.timestamp_subsec_nanos() as i64);
            Ok(now)
        }
    }
}

fn get_datetime_option(
    datetime_str: Option<String>,
) -> Result<Option<DateTime<Local>>, KimaiError> {
    match datetime_str {
        Some(s) => Ok(Some(str_to_datetime(&s)?)),
        None => Ok(None),
    }
}

#[tokio::main]
pub async fn print_begin_timesheet_record(
    config_path: Option<String>,
    user: Option<usize>,
    project: usize,
    activity: usize,
    begin: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;

    let record = begin_timesheet_record(
        &config,
        match user {
            Some(u) => u,
            None => get_current_user(&config).await?.id,
        },
        project,
        activity,
        get_datetime(begin)?,
        description,
        tags,
    )
    .await?;

    println!("Started new timesheet record:");
    record.print_table();

    Ok(())
}

#[tokio::main]
#[allow(clippy::too_many_arguments)]
pub async fn print_log_timesheet_record(
    config_path: Option<String>,
    user: Option<usize>,
    project: usize,
    activity: usize,
    begin: String,
    end: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;

    let record = log_timesheet_record(
        &config,
        match user {
            Some(u) => u,
            None => get_current_user(&config).await?.id,
        },
        project,
        activity,
        str_to_datetime(&begin)?,
        get_datetime_option(end)?,
        description,
        tags,
    )
    .await?;

    println!("Logged new timesheet record:");
    record.print_table();

    Ok(())
}

#[tokio::main]
pub async fn print_end_timesheet_record(
    config_path: Option<String>,
    id: usize,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;
    let record = end_timesheet_record(&config, id).await?;
    println!("Ended timesheet record:");
    record.print_table();

    Ok(())
}

#[tokio::main]
pub async fn print_active_timesheet(config_path: Option<String>) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;

    let records = get_active_timesheet(&config).await?;
    print_timesheet_entities(&records);

    Ok(())
}

#[tokio::main]
pub async fn print_recent_timesheet(
    config_path: Option<String>,
    user: Option<usize>,
    begin: Option<String>,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;

    let records =
        get_recent_timesheet(&config, user, begin.map(|b| str_to_datetime(&b).unwrap())).await?;
    print_timesheet_entities(&records);

    Ok(())
}

#[tokio::main]
pub async fn print_timesheet_record_status(
    config_path: Option<String>,
    id: usize,
) -> Result<(), KimaiError> {
    let config = load_config(config_path)?;

    let record = get_timesheet_record(&config, id).await?;
    record.print_table();

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn basic_test() {
        assert!(true);
    }
}
