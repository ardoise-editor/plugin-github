//! The REST API of GitHub, given in the shapes the panel draws: a pull
//! request is a `MergeRequest`, a run of GitHub Actions a `Pipeline`.

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};

/// The most pages of a list that are read (100 a page).
const MAX_PAGES: u32 = 10;
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The token is missing, wrong or expired.
    Unauthorized,
    /// What GitHub answered, with its status.
    Server(u16, String),
    /// The server was not reached, or its answer not read.
    Network(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unauthorized => write!(f, "{}", t!("gh.err_token")),
            Error::Server(status, said) => write!(f, "GitHub {status}: {said}"),
            Error::Network(why) => write!(f, "{}", t!("gh.err_network", why = why.as_str())),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct User {
    pub username: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Pipeline {
    pub id: u64,
    pub status: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub sha: String,
    pub web_url: String,
    pub created_at: String,
}

impl Pipeline {
    /// It can still change by itself.
    pub fn running(&self) -> bool {
        matches!(
            self.status.as_str(),
            "created" | "waiting_for_resource" | "preparing" | "pending" | "running"
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub stage: String,
    pub status: String,
    pub duration: Option<f64>,
    pub allow_failure: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct MergeRequest {
    pub iid: u64,
    pub title: String,
    pub description: Option<String>,
    /// `opened`, `closed`, `merged` or `locked`.
    pub state: String,
    pub draft: bool,
    pub source_branch: String,
    pub target_branch: String,
    pub author: User,
    pub web_url: String,
    pub updated_at: String,
    /// The commit at the head of the source: a merge asks for this one.
    pub sha: Option<String>,
    pub has_conflicts: bool,
    /// Why it can or can't be merged (`mergeable`, `ci_must_pass`…).
    pub detailed_merge_status: Option<String>,
    pub user_notes_count: u64,
    pub head_pipeline: Option<Pipeline>,
    /// The source branch is of this repository (not of a fork).
    pub same_repository: bool,
}

/// A file of a merge request.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    /// The hunks, without the lines that name the file.
    pub diff: String,
    pub new_file: bool,
    pub renamed_file: bool,
    pub deleted_file: bool,
    /// The server left the diff out: it is too big.
    pub too_large: bool,
}

impl FileDiff {
    /// The diff as `git diff` writes it, which is what the editor draws.
    pub fn patch(&self) -> String {
        let (old, new) = (&self.old_path, &self.new_path);
        let mut out = format!("diff --git a/{old} b/{new}\n");
        if self.new_file {
            out.push_str("new file mode 100644\n");
        }
        if self.deleted_file {
            out.push_str("deleted file mode 100644\n");
        }
        if self.renamed_file {
            out.push_str(&format!("rename from {old}\nrename to {new}\n"));
        }
        if !self.diff.is_empty() {
            let from = if self.new_file {
                "/dev/null".to_string()
            } else {
                format!("a/{old}")
            };
            let to = if self.deleted_file {
                "/dev/null".to_string()
            } else {
                format!("b/{new}")
            };
            out.push_str(&format!("--- {from}\n+++ {to}\n{}", self.diff));
            if !self.diff.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Position {
    pub new_path: Option<String>,
    pub old_path: Option<String>,
    pub new_line: Option<u64>,
    pub old_line: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Note {
    pub id: u64,
    pub body: String,
    pub author: User,
    pub created_at: String,
    /// Said by GitLab itself ("added 1 commit").
    pub system: bool,
    pub resolvable: bool,
    pub resolved: bool,
    /// The line of the diff it is about, if any.
    pub position: Option<Position>,
}

/// A thread of comments.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Discussion {
    pub id: String,
    pub notes: Vec<Note>,
}

impl Discussion {
    /// What people said (not GitLab's own lines).
    pub fn said(&self) -> bool {
        self.notes.iter().any(|n| !n.system)
    }
    pub fn resolvable(&self) -> bool {
        self.notes.iter().any(|n| n.resolvable)
    }
    pub fn resolved(&self) -> bool {
        self.resolvable() && self.notes.iter().all(|n| !n.resolvable || n.resolved)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Commit {
    pub short_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Tag {
    pub name: String,
    pub message: Option<String>,
    pub protected: bool,
    pub commit: Commit,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Branch {
    pub name: String,
    pub default: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ProjectInfo {
    pub default_branch: Option<String>,
}

/// A workflow of GitHub Actions.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Workflow {
    pub id: u64,
    pub name: String,
    pub state: String,
}

/// A merge request to make.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewMergeRequest {
    pub source: String,
    pub target: String,
    pub title: String,
    pub description: String,
    pub draft: bool,
    pub remove_source: bool,
    pub squash: bool,
}

/// How a merge request is merged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergeOptions {
    pub squash: bool,
    pub remove_source: bool,
    /// Merged by the server once the pipeline passes.
    pub when_pipeline_succeeds: bool,
}

/// A part of an address (`/` stays: names of branches and tags have some).
pub fn encode(part: &str) -> String {
    let mut out = String::new();
    for b in part.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn text_of(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn user_of(value: &Value) -> User {
    User {
        username: text_of(value, "login"),
        name: text_of(value, "name"),
    }
}

fn request_of(v: &Value) -> MergeRequest {
    let merged = v["merged"].as_bool() == Some(true) || v["merged_at"].is_string();
    let state = if merged {
        "merged"
    } else if v["state"] == "closed" {
        "closed"
    } else {
        "opened"
    };
    MergeRequest {
        iid: v["number"].as_u64().unwrap_or(0),
        title: text_of(v, "title"),
        description: v["body"].as_str().map(str::to_string),
        state: state.to_string(),
        draft: v["draft"].as_bool() == Some(true),
        source_branch: text_of(&v["head"], "ref"),
        target_branch: text_of(&v["base"], "ref"),
        author: user_of(&v["user"]),
        web_url: text_of(v, "html_url"),
        updated_at: text_of(v, "updated_at"),
        sha: v["head"]["sha"].as_str().map(str::to_string),
        // Not known until GitHub has worked it out (null).
        has_conflicts: v["mergeable"].as_bool() == Some(false),
        detailed_merge_status: v["mergeable_state"].as_str().map(str::to_string),
        user_notes_count: v["comments"].as_u64().unwrap_or(0)
            + v["review_comments"].as_u64().unwrap_or(0),
        head_pipeline: None,
        // The branch can be deleted after a merge only when it is of this
        // repository, not of a fork.
        same_repository: v["head"]["repo"]["full_name"].is_string()
            && v["head"]["repo"]["full_name"] == v["base"]["repo"]["full_name"],
    }
}

/// A run or a job, as the marks of the panel read it.
fn status_of(v: &Value) -> String {
    let status = text_of(v, "status");
    let said = if status == "completed" {
        text_of(v, "conclusion")
    } else {
        status
    };
    match said.as_str() {
        "failure" | "timed_out" | "startup_failure" => "failed".into(),
        "cancelled" => "canceled".into(),
        "in_progress" => "running".into(),
        "queued" | "requested" | "waiting" => "pending".into(),
        "action_required" => "manual".into(),
        _ => said,
    }
}

fn run_of(v: &Value) -> Pipeline {
    let mut reference = text_of(v, "head_branch");
    let name = text_of(v, "name");
    if !name.is_empty() {
        reference = format!("{name} · {reference}");
    }
    Pipeline {
        id: v["id"].as_u64().unwrap_or(0),
        status: status_of(v),
        reference,
        sha: text_of(v, "head_sha"),
        web_url: text_of(v, "html_url"),
        created_at: text_of(v, "created_at"),
    }
}

fn job_of(v: &Value) -> Job {
    let seconds = |key: &str| {
        v[key]
            .as_str()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
    };
    Job {
        id: v["id"].as_u64().unwrap_or(0),
        name: text_of(v, "name"),
        stage: text_of(v, "workflow_name"),
        status: status_of(v),
        duration: seconds("started_at")
            .zip(seconds("completed_at"))
            .map(|(from, to)| (to - from).num_seconds() as f64),
        allow_failure: false,
    }
}

fn file_of(v: &Value) -> FileDiff {
    let status = text_of(v, "status");
    let new_path = text_of(v, "filename");
    let renamed = status == "renamed";
    let diff = text_of(v, "patch");
    FileDiff {
        old_path: if renamed {
            text_of(v, "previous_filename")
        } else {
            new_path.clone()
        },
        new_file: status == "added",
        renamed_file: renamed,
        deleted_file: status == "removed",
        // GitHub leaves the patch out of a big or binary file.
        too_large: diff.is_empty() && v["changes"].as_u64().unwrap_or(0) > 0,
        new_path,
        diff,
    }
}

fn note_of(v: &Value, position: Option<Position>) -> Note {
    Note {
        id: v["id"].as_u64().unwrap_or(0),
        body: text_of(v, "body"),
        author: user_of(&v["user"]),
        created_at: v["created_at"]
            .as_str()
            .or_else(|| v["submitted_at"].as_str())
            .unwrap_or_default()
            .to_string(),
        system: false,
        // Resolving a thread is not part of this API.
        resolvable: false,
        resolved: false,
        position,
    }
}

/// The comments of a pull request as threads, in the order they began: those
/// of the conversation, what the reviews say, and those on lines (with their
/// answers). A thread on lines is named by its first comment.
pub fn threads(conversation: &[Value], reviews: &[Value], lines: &[Value]) -> Vec<Discussion> {
    let mut found: Vec<Discussion> = conversation
        .iter()
        .map(|c| Discussion {
            id: String::new(),
            notes: vec![note_of(c, None)],
        })
        .collect();
    for review in reviews {
        let state = text_of(review, "state");
        let mut note = note_of(review, None);
        let verdict = match state.as_str() {
            "APPROVED" => Some(t!("gh.review_approved")),
            "CHANGES_REQUESTED" => Some(t!("gh.review_changes")),
            _ => None,
        };
        if let Some(verdict) = verdict {
            note.body = format!("{verdict}\n{}", note.body).trim().to_string();
        }
        if !note.body.is_empty() && state != "PENDING" {
            found.push(Discussion {
                id: String::new(),
                notes: vec![note],
            });
        }
    }
    for comment in lines {
        let line = |key: &str| comment[key].as_u64();
        let position = Position {
            new_path: comment["path"].as_str().map(str::to_string),
            old_path: None,
            new_line: line("line").or_else(|| line("original_line")),
            old_line: None,
        };
        let note = note_of(comment, Some(position));
        let root = comment["in_reply_to_id"]
            .as_u64()
            .unwrap_or(note.id)
            .to_string();
        match found.iter_mut().find(|d| d.id == root) {
            Some(thread) => thread.notes.push(note),
            None => found.push(Discussion {
                id: root,
                notes: vec![note],
            }),
        }
    }
    found.sort_by(|a, b| {
        let at = |d: &Discussion| d.notes.first().map(|n| n.created_at.clone());
        at(a).cmp(&at(b))
    });
    found
}

/// What GitHub says went wrong.
fn message_of(body: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return body.chars().take(300).collect();
    };
    let mut said = text_of(&value, "message");
    if let Some(errors) = value["errors"].as_array() {
        for e in errors {
            let detail = e
                .as_str()
                .or_else(|| e["message"].as_str())
                .unwrap_or_default();
            if !detail.is_empty() {
                said.push_str(&format!(" — {detail}"));
            }
        }
    }
    said
}

/// The API of one repository, with the token of its server.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    /// `https://api.github.com`, or `https://host/api/v3` (Enterprise).
    base: String,
    token: String,
    /// `owner/name`.
    project: String,
}

/// An answer: its text, whether more pages follow, and where it sends.
struct Answer {
    text: String,
    more: bool,
    location: Option<String>,
}

impl Client {
    /// `server`: the address of the web interface (`https://github.com`).
    /// Only `https` carries a token, but to this machine itself.
    pub fn new(server: &str, project: &str, token: &str) -> Result<Client, Error> {
        let server = server.trim_end_matches('/');
        let local = ["http://localhost", "http://127.0.0.1"].iter().any(|l| {
            server
                .strip_prefix(l)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with([':', '/']))
        });
        if !server.starts_with("https://") && !local {
            return Err(Error::Network(t!("gh.err_https").to_string()));
        }
        let base = if server == "https://github.com" {
            "https://api.github.com".to_string()
        } else {
            format!("{server}/api/v3")
        };
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .user_agent("ardoise-github")
            // An answer that sends elsewhere is not followed: the token
            // would go with it.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Client {
            http,
            base,
            token: token.trim().to_string(),
            project: encode(project),
        })
    }

    async fn send(&self, method: Method, path: &str, body: Option<Value>) -> Result<Answer, Error> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28")
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|e| Error::Network(e.without_url().to_string()))?;
        let status = response.status();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let more = header("link").is_some_and(|l| l.contains("rel=\"next\""));
        let location = header("location");
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(e.without_url().to_string()))?;
        if status == StatusCode::UNAUTHORIZED {
            return Err(Error::Unauthorized);
        }
        if status.is_redirection() && location.is_some() {
            return Ok(Answer {
                text,
                more: false,
                location,
            });
        }
        if !status.is_success() {
            return Err(Error::Server(status.as_u16(), message_of(&text)));
        }
        Ok(Answer {
            text,
            more,
            location: None,
        })
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value, Error> {
        let answer = self.send(method, path, body).await?;
        // A renamed repository is answered by a redirect.
        if answer.location.is_some() {
            return Err(Error::Network(t!("gh.err_moved").to_string()));
        }
        serde_json::from_str(&answer.text).map_err(|e| Error::Network(e.to_string()))
    }

    async fn act(&self, method: Method, path: &str, body: Option<Value>) -> Result<(), Error> {
        self.send(method, path, body).await.map(|_| ())
    }

    /// Every page of a list, or of the list under `key`. `path` has a `?`.
    async fn list(&self, path: &str, key: Option<&str>, pages: u32) -> Result<Vec<Value>, Error> {
        let mut all = Vec::new();
        for page in 1..=pages {
            let answer = self
                .send(
                    Method::GET,
                    &format!("{path}&per_page=100&page={page}"),
                    None,
                )
                .await?;
            let value: Value =
                serde_json::from_str(&answer.text).map_err(|e| Error::Network(e.to_string()))?;
            let items = match key {
                Some(key) => value[key].as_array(),
                None => value.as_array(),
            };
            all.extend(items.cloned().unwrap_or_default());
            if !answer.more {
                break;
            }
        }
        Ok(all)
    }

    fn at(&self, rest: &str) -> String {
        format!("/repos/{}{rest}", self.project)
    }

    fn pr(&self, number: u64) -> String {
        self.at(&format!("/pulls/{number}"))
    }

    pub async fn me(&self) -> Result<User, Error> {
        Ok(user_of(&self.call(Method::GET, "/user", None).await?))
    }

    pub async fn project(&self) -> Result<ProjectInfo, Error> {
        let v = self.call(Method::GET, &self.at(""), None).await?;
        Ok(ProjectInfo {
            default_branch: v["default_branch"].as_str().map(str::to_string),
        })
    }

    pub async fn branches(&self) -> Result<Vec<Branch>, Error> {
        let found = self.list(&self.at("/branches?"), None, MAX_PAGES).await?;
        Ok(found
            .iter()
            .map(|b| Branch {
                name: text_of(b, "name"),
                default: false,
            })
            .collect())
    }

    // ─── Pull requests ──────────────────────────────────────────────────

    /// `state`: `opened`, `merged`, `closed` or `all`, latest changed first.
    pub async fn merge_requests(&self, state: &str) -> Result<Vec<MergeRequest>, Error> {
        // GitHub knows open and closed: merged are among the closed.
        let asked = match state {
            "opened" => "open",
            "all" => "all",
            _ => "closed",
        };
        let path = self.at(&format!("/pulls?state={asked}&sort=updated&direction=desc"));
        let found = self.list(&path, None, 2).await?;
        Ok(found
            .iter()
            .map(request_of)
            .filter(|m| state == "all" || m.state == state)
            .collect())
    }

    pub async fn merge_request(&self, number: u64) -> Result<MergeRequest, Error> {
        Ok(request_of(
            &self.call(Method::GET, &self.pr(number), None).await?,
        ))
    }

    pub async fn create(&self, new: &NewMergeRequest) -> Result<MergeRequest, Error> {
        let body = json!({
            "head": new.source.trim(),
            "base": new.target.trim(),
            "title": new.title.trim(),
            "body": new.description,
            "draft": new.draft,
        });
        Ok(request_of(
            &self
                .call(Method::POST, &self.at("/pulls"), Some(body))
                .await?,
        ))
    }

    pub async fn set_open(&self, number: u64, open: bool) -> Result<(), Error> {
        let state = if open { "open" } else { "closed" };
        self.act(
            Method::PATCH,
            &self.pr(number),
            Some(json!({ "state": state })),
        )
        .await
    }

    /// Merges. `sha`: the head that was looked at; GitHub refuses if the
    /// branch moved since. `branch`: the one to delete once merged.
    pub async fn merge(
        &self,
        number: u64,
        sha: &str,
        how: &MergeOptions,
        branch: Option<&str>,
    ) -> Result<(), Error> {
        let method = if how.squash { "squash" } else { "merge" };
        self.act(
            Method::PUT,
            &format!("{}/merge", self.pr(number)),
            Some(json!({ "sha": sha, "merge_method": method })),
        )
        .await?;
        match branch.filter(|_| how.remove_source) {
            Some(branch) => {
                let path = self.at(&format!("/git/refs/heads/{}", encode(branch)));
                match self.act(Method::DELETE, &path, None).await {
                    // Already deleted by the repository's own setting. Another
                    // refusal (a protected branch) is said.
                    Err(Error::Server(404, _)) => Ok(()),
                    Err(Error::Server(422, said)) if said.contains("does not exist") => Ok(()),
                    other => other.map_err(|e| {
                        Error::Network(
                            t!("gh.err_branch_kept", why = e.to_string().as_str()).to_string(),
                        )
                    }),
                }
            }
            None => Ok(()),
        }
    }

    /// A review of the commit seen: `APPROVE` or `REQUEST_CHANGES` (which
    /// needs words).
    pub async fn review(
        &self,
        number: u64,
        sha: &str,
        event: &str,
        body: &str,
    ) -> Result<(), Error> {
        self.act(
            Method::POST,
            &format!("{}/reviews", self.pr(number)),
            Some(json!({"commit_id": sha, "event": event, "body": body})),
        )
        .await
    }

    pub async fn diffs(&self, number: u64) -> Result<Vec<FileDiff>, Error> {
        let path = format!("{}/files?", self.pr(number));
        Ok(self
            .list(&path, None, MAX_PAGES)
            .await?
            .iter()
            .map(file_of)
            .collect())
    }

    pub async fn discussions(&self, number: u64) -> Result<Vec<Discussion>, Error> {
        let conversation = self
            .list(
                &self.at(&format!("/issues/{number}/comments?")),
                None,
                MAX_PAGES,
            )
            .await?;
        let reviews = self
            .list(&format!("{}/reviews?", self.pr(number)), None, MAX_PAGES)
            .await?;
        let lines = self
            .list(&format!("{}/comments?", self.pr(number)), None, MAX_PAGES)
            .await?;
        Ok(threads(&conversation, &reviews, &lines))
    }

    pub async fn comment(&self, number: u64, body: &str) -> Result<(), Error> {
        self.act(
            Method::POST,
            &self.at(&format!("/issues/{number}/comments")),
            Some(json!({ "body": body })),
        )
        .await
    }

    /// An answer in a thread on lines.
    pub async fn reply(&self, number: u64, thread: &str, body: &str) -> Result<(), Error> {
        let path = format!("{}/comments/{}/replies", self.pr(number), encode(thread));
        self.act(Method::POST, &path, Some(json!({ "body": body })))
            .await
    }

    // ─── GitHub Actions ─────────────────────────────────────────────────

    async fn runs(&self, query: &str) -> Result<Vec<Pipeline>, Error> {
        let path = self.at(&format!("/actions/runs?{query}"));
        Ok(self
            .list(&path, Some("workflow_runs"), 1)
            .await?
            .iter()
            .map(run_of)
            .collect())
    }

    /// The runs for the commit at the head of a pull request.
    pub async fn commit_pipelines(&self, sha: &str) -> Result<Vec<Pipeline>, Error> {
        self.runs(&format!("head_sha={}", encode(sha))).await
    }

    pub async fn pipelines(&self) -> Result<Vec<Pipeline>, Error> {
        self.runs("exclude_pull_requests=false").await
    }

    pub async fn jobs(&self, run: u64) -> Result<Vec<Job>, Error> {
        let path = self.at(&format!("/actions/runs/{run}/jobs?filter=latest"));
        Ok(self
            .list(&path, Some("jobs"), MAX_PAGES)
            .await?
            .iter()
            .map(job_of)
            .collect())
    }

    pub async fn workflows(&self) -> Result<Vec<Workflow>, Error> {
        let found = self
            .list(&self.at("/actions/workflows?"), Some("workflows"), 1)
            .await?;
        Ok(found
            .iter()
            .filter_map(|w| serde_json::from_value::<Workflow>(w.clone()).ok())
            .filter(|w| w.state == "active")
            .collect())
    }

    /// Starts a workflow by hand (it must say `workflow_dispatch`).
    pub async fn run_workflow(&self, workflow: u64, reference: &str) -> Result<(), Error> {
        self.act(
            Method::POST,
            &self.at(&format!("/actions/workflows/{workflow}/dispatches")),
            Some(json!({ "ref": reference.trim() })),
        )
        .await
    }

    /// `retry` (the jobs that failed) or `cancel`.
    pub async fn pipeline_action(&self, run: u64, action: &str) -> Result<(), Error> {
        let verb = if action == "retry" {
            "rerun-failed-jobs"
        } else {
            "cancel"
        };
        let path = self.at(&format!("/actions/runs/{run}/{verb}"));
        self.act(Method::POST, &path, None).await
    }

    pub async fn delete_pipeline(&self, run: u64) -> Result<(), Error> {
        self.act(
            Method::DELETE,
            &self.at(&format!("/actions/runs/{run}")),
            None,
        )
        .await
    }

    pub async fn rerun_job(&self, job: u64) -> Result<(), Error> {
        let path = self.at(&format!("/actions/jobs/{job}/rerun"));
        self.act(Method::POST, &path, None).await
    }

    /// What a job wrote. GitHub sends to where the file is, which is read
    /// without the token: the address carries its own right.
    pub async fn job_log(&self, job: u64) -> Result<String, Error> {
        let path = self.at(&format!("/actions/jobs/{job}/logs"));
        let answer = self.send(Method::GET, &path, None).await?;
        let Some(location) = answer.location else {
            return Ok(answer.text);
        };
        let failed = |e: reqwest::Error| Error::Network(e.without_url().to_string());
        let response = self.http.get(location).send().await.map_err(failed)?;
        if !response.status().is_success() {
            return Err(Error::Server(response.status().as_u16(), String::new()));
        }
        response.text().await.map_err(failed)
    }

    // ─── Tags ───────────────────────────────────────────────────────────

    pub async fn tags(&self) -> Result<Vec<Tag>, Error> {
        let found = self.list(&self.at("/tags?"), None, 2).await?;
        Ok(found
            .iter()
            .map(|t| Tag {
                name: text_of(t, "name"),
                message: None,
                protected: false,
                commit: Commit {
                    short_id: text_of(&t["commit"], "sha").chars().take(8).collect(),
                    title: String::new(),
                },
            })
            .collect())
    }

    /// A tag at `reference`; with a message it is annotated.
    pub async fn create_tag(
        &self,
        name: &str,
        reference: &str,
        message: &str,
    ) -> Result<(), Error> {
        let commit = self
            .call(
                Method::GET,
                &self.at(&format!("/commits/{}", encode(reference.trim()))),
                None,
            )
            .await?;
        let mut sha = text_of(&commit, "sha");
        if !message.trim().is_empty() {
            let tag = self
                .call(
                    Method::POST,
                    &self.at("/git/tags"),
                    Some(json!({"tag": name.trim(), "message": message.trim(),
                                "object": sha, "type": "commit"})),
                )
                .await?;
            sha = text_of(&tag, "sha");
        }
        self.act(
            Method::POST,
            &self.at("/git/refs"),
            Some(json!({"ref": format!("refs/tags/{}", name.trim()), "sha": sha})),
        )
        .await
    }

    pub async fn delete_tag(&self, name: &str) -> Result<(), Error> {
        let path = self.at(&format!("/git/refs/tags/{}", encode(name)));
        self.act(Method::DELETE, &path, None).await
    }
}

#[cfg(test)]
pub mod mock {
    //! A server of a few lines that answers what a test tells it to, and
    //! keeps what it was asked.

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    pub struct Asked {
        /// `GET /api/v4/user`
        pub line: String,
        pub authorization: String,
        pub body: String,
    }

    pub struct Server {
        pub address: String,
        pub asked: Arc<Mutex<Vec<Asked>>>,
    }

    /// `answers`: for each request in turn, the status, more headers and the
    /// body.
    pub fn serve(answers: Vec<(u16, &'static str, &'static str)>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let seen = asked.clone();
        std::thread::spawn(move || {
            for (status, headers, body) in answers {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (mut authorization, mut length) = (String::new(), 0);
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header.trim().is_empty() {
                        break;
                    }
                    let (name, value) = header.split_once(':').unwrap();
                    match name.to_lowercase().as_str() {
                        "authorization" => authorization = value.trim().to_string(),
                        "content-length" => length = value.trim().parse().unwrap(),
                        _ => {}
                    }
                }
                let mut sent = vec![0; length];
                reader.read_exact(&mut sent).unwrap();
                seen.lock().unwrap().push(Asked {
                    line: line.trim_end().trim_end_matches(" HTTP/1.1").to_string(),
                    authorization,
                    body: String::from_utf8(sent).unwrap(),
                });
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
                    body.len()
                );
            }
        });
        Server { address, asked }
    }

    impl Server {
        pub fn lines(&self) -> Vec<String> {
            self.asked
                .lock()
                .unwrap()
                .iter()
                .map(|a| a.line.clone())
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    fn client(server: &mock::Server) -> Client {
        Client::new(&server.address, "me/app", "tok").unwrap()
    }

    #[test]
    fn github_has_its_api_apart_and_enterprise_beside_the_site() {
        let at = |server: &str| Client::new(server, "a/b", "t").unwrap().base;
        assert_eq!(at("https://github.com"), "https://api.github.com");
        assert_eq!(at("https://git.corp.com/"), "https://git.corp.com/api/v3");
        assert!(Client::new("http://github.com", "a/b", "t").is_err());
    }

    #[test]
    fn merged_pull_requests_are_told_from_the_closed_ones() {
        let body = r#"[{"number": 1, "state": "closed", "merged_at": "2026-01-01T00:00:00Z",
              "head": {"ref": "a", "sha": "s", "repo": {"full_name": "me/app"}},
              "base": {"ref": "main", "repo": {"full_name": "me/app"}}},
             {"number": 2, "state": "closed", "merged_at": null,
              "head": {"ref": "b", "repo": {"full_name": "other/app"}},
              "base": {"ref": "main", "repo": {"full_name": "me/app"}}}]"#;
        let server = mock::serve(vec![(200, "", body), (200, "", body)]);
        let api = client(&server);
        let merged = block(api.merge_requests("merged")).unwrap();
        assert_eq!(merged.len(), 1);
        assert!(merged[0].same_repository);
        let closed = block(api.merge_requests("closed")).unwrap();
        assert_eq!(closed[0].iid, 2);
        assert!(!closed[0].same_repository, "a fork's branch is not ours");
        assert!(server.lines()[0].starts_with("GET /api/v3/repos/me/app/pulls?state=closed"));
    }

    #[test]
    fn a_merge_asks_for_the_commit_seen_then_deletes_the_branch() {
        let server = mock::serve(vec![(200, "", "{}"), (204, "", "")]);
        let how = MergeOptions {
            squash: true,
            remove_source: true,
            ..Default::default()
        };
        block(client(&server).merge(3, "abc", &how, Some("feat/x"))).unwrap();
        let asked = server.asked.lock().unwrap().clone();
        let sent: Value = serde_json::from_str(&asked[0].body).unwrap();
        assert_eq!(
            (sent["sha"].as_str(), sent["merge_method"].as_str()),
            (Some("abc"), Some("squash"))
        );
        assert_eq!(
            asked[1].line,
            "DELETE /api/v3/repos/me/app/git/refs/heads/feat/x"
        );
        assert_eq!(asked[0].authorization, "Bearer tok");
    }

    #[test]
    fn a_refused_merge_deletes_nothing() {
        let server = mock::serve(vec![
            (409, "", r#"{"message": "Head branch was modified"}"#),
            (204, "", ""),
        ]);
        let how = MergeOptions {
            remove_source: true,
            ..Default::default()
        };
        assert_eq!(
            block(client(&server).merge(3, "old", &how, Some("x"))),
            Err(Error::Server(409, "Head branch was modified".into()))
        );
        assert_eq!(server.lines().len(), 1);
    }

    #[test]
    fn a_branch_github_will_not_delete_is_said() {
        let server = mock::serve(vec![
            (200, "", "{}"),
            (
                422,
                "",
                r#"{"message": "Cannot delete a protected branch"}"#,
            ),
            (200, "", "{}"),
            (422, "", r#"{"message": "Reference does not exist"}"#),
        ]);
        let api = client(&server);
        let how = MergeOptions {
            remove_source: true,
            ..Default::default()
        };
        assert!(block(api.merge(1, "a", &how, Some("develop"))).is_err());
        assert!(block(api.merge(2, "b", &how, Some("gone"))).is_ok());
    }

    #[test]
    fn the_log_of_a_job_is_read_where_it_is_without_the_token() {
        let store = mock::serve(vec![(200, "", "line one")]);
        let location: &'static str =
            Box::leak(format!("Location: {}/blob?sig=1\r\n", store.address).into_boxed_str());
        let server = mock::serve(vec![(302, location, "")]);
        assert_eq!(block(client(&server).job_log(4)).unwrap(), "line one");
        assert_eq!(store.asked.lock().unwrap()[0].authorization, "");
    }

    #[test]
    fn an_annotated_tag_is_made_in_two_steps() {
        let server = mock::serve(vec![
            (200, "", r#"{"sha": "c1"}"#),
            (201, "", r#"{"sha": "t1"}"#),
            (201, "", "{}"),
        ]);
        block(client(&server).create_tag("v1", "main", "first")).unwrap();
        let asked = server.asked.lock().unwrap().clone();
        assert!(asked[2].line.ends_with("/git/refs"));
        let sent: Value = serde_json::from_str(&asked[2].body).unwrap();
        assert_eq!(
            (sent["ref"].as_str(), sent["sha"].as_str()),
            (Some("refs/tags/v1"), Some("t1"))
        );
    }

    #[test]
    fn comments_become_threads_in_order() {
        let conversation = [json!({"id": 1, "body": "hello", "created_at": "2026-01-02"})];
        let reviews = [
            json!({"id": 2, "body": "", "state": "APPROVED", "submitted_at": "2026-01-04"}),
            json!({"id": 3, "body": "", "state": "COMMENTED", "submitted_at": "2026-01-04"}),
        ];
        let lines = [
            json!({"id": 10, "body": "why?", "path": "a.rs", "line": 3, "created_at": "2026-01-01"}),
            json!({"id": 11, "body": "because", "in_reply_to_id": 10, "path": "a.rs", "created_at": "2026-01-03"}),
        ];
        let found = threads(&conversation, &reviews, &lines);
        assert_eq!(found.len(), 3, "an empty comment review says nothing");
        assert_eq!(found[0].id, "10");
        assert_eq!(found[0].notes.len(), 2);
        assert_eq!(found[1].notes[0].body, "hello");
        assert!(found[2].notes[0]
            .body
            .contains(&t!("gh.review_approved").to_string()));
    }

    #[test]
    fn runs_say_their_state_in_the_words_of_the_panel() {
        let state = |status: &str, conclusion: Option<&str>| {
            status_of(&json!({"status": status, "conclusion": conclusion}))
        };
        assert_eq!(state("completed", Some("failure")), "failed");
        assert_eq!(state("completed", Some("success")), "success");
        assert_eq!(state("in_progress", None), "running");
        assert_eq!(state("queued", None), "pending");
        assert!(run_of(&json!({"status": "queued"})).running());
    }

    #[test]
    fn a_file_without_patch_that_changed_is_too_big_to_show() {
        let big = file_of(&json!({"filename": "x", "status": "modified", "changes": 9000}));
        assert!(big.too_large);
        let moved = file_of(
            &json!({"filename": "n", "previous_filename": "o", "status": "renamed", "changes": 0}),
        );
        assert!(moved.renamed_file && !moved.too_large);
        assert!(moved.patch().contains("rename from o\nrename to n"));
    }
}
