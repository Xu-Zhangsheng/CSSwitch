//! Static, non-executing inspection result contract for GitHub Skill archives.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Cursor, Read};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::archive::{inspect_archive, EntryMeta};
use crate::{CATALOG_STAMP_FILE, IMPORT_ORIGIN_FILE};

pub const INSPECTION_REPORT_SCHEMA: &str = "csswitch.package-inspection.v1";
pub const CALLER_ASSERTED_UNVERIFIED: &str = "caller_asserted_unverified";
const INSPECTION_GRAPH_SCHEMA: &str = "csswitch.component-graph.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GithubInspectionSource {
    pub owner: String,
    pub repo: String,
    pub commit_sha: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GithubSourceClaim {
    pub owner: String,
    pub repo: String,
    pub commit_sha: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    pub verification: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectionReportV1 {
    pub schema: String,
    pub source_claim: GithubSourceClaim,
    pub outcome: InspectionOutcome,
    pub disposition: String,
    pub package: PackageSummary,
    pub graph: ComponentGraph,
    pub findings: Vec<InspectionFinding>,
    pub limits: InspectionLimits,
    pub effects: InspectionEffects,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InspectionOutcome {
    Complete,
    Partial,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSummary {
    pub kind: String,
    pub content_sha256: String,
    pub file_count: usize,
    pub total_bytes: usize,
    pub executable_file_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentGraph {
    pub schema: String,
    pub sha256: String,
    pub nodes: Vec<ComponentNode>,
    pub edges: Vec<ComponentEdge>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentNode {
    pub id: String,
    pub kind: ComponentKind,
    pub source_path: String,
    pub local_name: String,
    pub compatibility_status: CompatibilityStatus,
    pub confirmation_required: bool,
    pub executable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_preapproved_tools: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ComponentKind {
    Package,
    Skill,
    McpServer,
    Executable,
    Asset,
    UnknownHostComponent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompatibilityStatus {
    Native,
    Adapted,
    Degraded,
    Unsupported,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentEdge {
    pub from: String,
    pub relation: String,
    pub to: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct InspectionFinding {
    pub severity: InspectionSeverity,
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InspectionSeverity {
    Info,
    Warning,
    Blocking,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectionLimits {
    pub profile: String,
    pub raw_archive_bytes: usize,
    pub archive_entries: usize,
    pub files: usize,
    pub single_file_bytes: usize,
    pub expanded_total_bytes: usize,
    pub path_bytes: usize,
    pub path_depth: usize,
    pub compression_ratio: u64,
    pub manifest_bytes: usize,
    pub mcp_manifests: usize,
    pub declared_tools_per_skill: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub findings: usize,
}

impl Default for InspectionLimits {
    fn default() -> Self {
        Self {
            profile: "github_skill_archive_inspect_v1".into(),
            raw_archive_bytes: 128 * 1024 * 1024,
            archive_entries: 10_000,
            files: 2_000,
            single_file_bytes: 4 * 1024 * 1024,
            expanded_total_bytes: 64 * 1024 * 1024,
            path_bytes: 1_024,
            path_depth: 32,
            compression_ratio: 200,
            manifest_bytes: 64 * 1024,
            mcp_manifests: 128,
            declared_tools_per_skill: 128,
            graph_nodes: 4_096,
            graph_edges: 8_192,
            findings: 1_024,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectionEffects {
    pub network_access: String,
    pub filesystem_mutation: String,
    pub apply: String,
    pub install: String,
    pub science_attach: String,
    pub mcp_launch: String,
    pub credential_access: String,
}

impl Default for InspectionEffects {
    fn default() -> Self {
        Self {
            network_access: "not_performed".into(),
            filesystem_mutation: "not_performed".into(),
            apply: "not_run".into(),
            install: "not_run".into(),
            science_attach: "not_run".into(),
            mcp_launch: "not_run".into(),
            credential_access: "not_performed".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InspectionError {
    pub code: String,
    pub phase: String,
    pub quarantined: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<u64>,
}

impl fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} during {}", self.code, self.phase)
    }
}

impl std::error::Error for InspectionError {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct SkillFrontmatter {
    name: String,
    description: String,
    allowed_tools: Option<String>,
}

#[derive(Clone, Debug)]
struct QuarantinedFile {
    path: String,
    content: Vec<u8>,
    executable: bool,
}

#[derive(Clone, Debug)]
struct QuarantinedPackage {
    files: Vec<QuarantinedFile>,
    content_sha256: String,
    total_bytes: usize,
    findings: Vec<InspectionFinding>,
}

/// Inspects already-fetched ZIP bytes without network, filesystem, process,
/// credential, install, apply, or Science effects. The report never executes
/// archive content.
pub fn inspect_github_skill_archive(
    source: &GithubInspectionSource,
    bytes: &[u8],
) -> Result<InspectionReportV1, InspectionError> {
    inspect_github_skill_archive_with_limits(source, bytes, InspectionLimits::default())
}

fn inspect_github_skill_archive_with_limits(
    source: &GithubInspectionSource,
    bytes: &[u8],
    limits: InspectionLimits,
) -> Result<InspectionReportV1, InspectionError> {
    let source = validate_source(source)?;
    let package = quarantine_archive(bytes, &source.path, &limits)?;
    let mut findings = package.findings;
    let graph = build_graph(&package.files, &mut findings, &limits)?;
    let outcome = if findings
        .iter()
        .any(|finding| finding.severity == InspectionSeverity::Blocking)
    {
        InspectionOutcome::Partial
    } else {
        InspectionOutcome::Complete
    };

    Ok(InspectionReportV1 {
        schema: INSPECTION_REPORT_SCHEMA.into(),
        source_claim: GithubSourceClaim {
            owner: source.owner,
            repo: source.repo,
            commit_sha: source.commit_sha,
            path: source.path,
            verification: CALLER_ASSERTED_UNVERIFIED.into(),
        },
        outcome,
        disposition: "quarantined_inspect_only".into(),
        package: PackageSummary {
            kind: "github_skill_package".into(),
            content_sha256: package.content_sha256,
            file_count: package.files.len(),
            total_bytes: package.total_bytes,
            executable_file_count: package.files.iter().filter(|file| file.executable).count(),
        },
        graph,
        findings,
        limits,
        effects: InspectionEffects::default(),
    })
}

fn build_graph(
    files: &[QuarantinedFile],
    findings: &mut Vec<InspectionFinding>,
    limits: &InspectionLimits,
) -> Result<ComponentGraph, InspectionError> {
    let package_id = component_id("package", "");
    let mut nodes = vec![component_node(
        package_id.clone(),
        ComponentKind::Package,
        "",
        "package",
        CompatibilityStatus::Unsupported,
        false,
        false,
    )];
    let mut edges = Vec::new();
    let mut mcp_manifests = 0usize;
    for file in files {
        let mut children = classify_file(file, findings, limits)?;
        let mcp_manifest_nodes = children
            .iter()
            .filter(|child| child.kind == ComponentKind::McpServer)
            .count();
        ensure_collection_limit(
            "MCP_MANIFEST_LIMIT",
            "graph",
            limits.mcp_manifests,
            mcp_manifests,
            mcp_manifest_nodes,
        )?;
        ensure_collection_limit(
            "GRAPH_NODE_LIMIT",
            "graph",
            limits.graph_nodes,
            nodes.len(),
            children.len(),
        )?;
        ensure_collection_limit(
            "GRAPH_EDGE_LIMIT",
            "graph",
            limits.graph_edges,
            edges.len(),
            children.len(),
        )?;
        for child in &children {
            edges.push(ComponentEdge {
                from: package_id.clone(),
                relation: "contains".into(),
                to: child.id.clone(),
            });
        }
        nodes.append(&mut children);
        mcp_manifests = mcp_manifests
            .checked_add(mcp_manifest_nodes)
            .ok_or_else(|| inspection_error("GRAPH_COUNT_OVERFLOW", "graph", true))?;
    }
    ensure_collection_limit(
        "MCP_MANIFEST_LIMIT",
        "graph",
        limits.mcp_manifests,
        mcp_manifests,
        0,
    )?;
    ensure_collection_limit(
        "GRAPH_NODE_LIMIT",
        "graph",
        limits.graph_nodes,
        nodes.len(),
        0,
    )?;
    ensure_collection_limit(
        "GRAPH_EDGE_LIMIT",
        "graph",
        limits.graph_edges,
        edges.len(),
        0,
    )?;
    nodes.sort();
    edges.sort();
    let graph_bytes = serde_json::to_vec(&(&nodes, &edges))
        .map_err(|_| inspection_error("GRAPH_SERIALIZATION_FAILED", "graph", true))?;
    Ok(ComponentGraph {
        schema: INSPECTION_GRAPH_SCHEMA.into(),
        sha256: format!("{:x}", Sha256::digest(graph_bytes)),
        nodes,
        edges,
    })
}

fn classify_file(
    file: &QuarantinedFile,
    findings: &mut Vec<InspectionFinding>,
    limits: &InspectionLimits,
) -> Result<Vec<ComponentNode>, InspectionError> {
    let mut nodes = Vec::new();
    if is_skill_manifest(&file.path) {
        nodes.push(skill_node(file, findings, limits)?);
    } else if file.path.ends_with(".mcp.json") {
        nodes.push(mcp_node(file, findings, limits)?);
    } else if is_unknown_manifest(&file.path) {
        push_finding(
            findings,
            finding(
                InspectionSeverity::Warning,
                "UNKNOWN_HOST_MANIFEST",
                &file.path,
            ),
            limits,
            "graph",
        )?;
        nodes.push(component_for_file(
            file,
            ComponentKind::UnknownHostComponent,
            CompatibilityStatus::Unsupported,
            true,
        ));
    } else {
        nodes.push(component_for_file(
            file,
            ComponentKind::Asset,
            CompatibilityStatus::Unsupported,
            false,
        ));
    }
    if file.executable {
        nodes.push(component_for_file(
            file,
            ComponentKind::Executable,
            CompatibilityStatus::Unsupported,
            true,
        ));
    }
    Ok(nodes)
}

fn skill_node(
    file: &QuarantinedFile,
    findings: &mut Vec<InspectionFinding>,
    limits: &InspectionLimits,
) -> Result<ComponentNode, InspectionError> {
    let parsed = frontmatter_slice(&file.content)
        .and_then(|slice| serde_saphyr::from_str::<SkillFrontmatter>(slice).ok())
        .filter(|frontmatter| valid_frontmatter(frontmatter, &file.path));
    match parsed {
        Some(frontmatter) => match declared_tools(&frontmatter, limits) {
            Some(declared_preapproved_tools) => {
                let mut node = component_node(
                    component_id("skill", &file.path),
                    ComponentKind::Skill,
                    &file.path,
                    &frontmatter.name,
                    CompatibilityStatus::Adapted,
                    false,
                    file.executable,
                );
                node.declared_preapproved_tools = declared_preapproved_tools;
                Ok(node)
            }
            None => {
                push_finding(
                    findings,
                    finding(
                        InspectionSeverity::Blocking,
                        "SKILL_ALLOWED_TOOLS_UNSUPPORTED",
                        &file.path,
                    ),
                    limits,
                    "graph",
                )?;
                Ok(component_for_file(
                    file,
                    ComponentKind::Skill,
                    CompatibilityStatus::Unsupported,
                    false,
                ))
            }
        },
        None => {
            push_finding(
                findings,
                finding(
                    InspectionSeverity::Blocking,
                    "SKILL_FRONTMATTER_UNSUPPORTED",
                    &file.path,
                ),
                limits,
                "graph",
            )?;
            Ok(component_for_file(
                file,
                ComponentKind::Skill,
                CompatibilityStatus::Unsupported,
                false,
            ))
        }
    }
}

fn mcp_node(
    file: &QuarantinedFile,
    findings: &mut Vec<InspectionFinding>,
    limits: &InspectionLimits,
) -> Result<ComponentNode, InspectionError> {
    let valid_object = serde_json::from_slice::<serde_json::Value>(&file.content)
        .map(|value| value.is_object())
        .unwrap_or(false);
    if !valid_object {
        push_finding(
            findings,
            finding(
                InspectionSeverity::Blocking,
                "MCP_MANIFEST_UNSUPPORTED",
                &file.path,
            ),
            limits,
            "graph",
        )?;
    }
    Ok(component_for_file(
        file,
        ComponentKind::McpServer,
        CompatibilityStatus::Unsupported,
        true,
    ))
}

fn declared_tools(
    frontmatter: &SkillFrontmatter,
    limits: &InspectionLimits,
) -> Option<Vec<String>> {
    let mut tools = Vec::new();
    for tool in frontmatter
        .allowed_tools
        .as_deref()
        .into_iter()
        .flat_map(str::split_whitespace)
    {
        if tools.len() >= limits.declared_tools_per_skill || !safe_declared_tool(tool) {
            return None;
        }
        tools.push(tool.to_owned());
    }
    Some(tools)
}

fn safe_declared_tool(tool: &str) -> bool {
    !tool.is_empty()
        && tool.len() <= 100
        && tool
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn is_skill_manifest(path: &str) -> bool {
    path == "SKILL.md" || path.ends_with("/SKILL.md")
}

fn is_unknown_manifest(path: &str) -> bool {
    matches!(
        path.rsplit('/').next(),
        Some("plugin.json" | "package.json" | "manifest.json")
    ) || path.ends_with(".manifest.json")
}

fn frontmatter_slice(content: &[u8]) -> Option<&str> {
    let opening_length = if content.starts_with(b"---\r\n") {
        5
    } else if content.starts_with(b"---\n") {
        4
    } else {
        return None;
    };
    let mut line_start = opening_length;
    while line_start < content.len() {
        let line_end = content[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| line_start + offset)
            .unwrap_or(content.len());
        let line = content[line_start..line_end]
            .strip_suffix(b"\r")
            .unwrap_or(&content[line_start..line_end]);
        if line == b"---" {
            return std::str::from_utf8(&content[opening_length..line_start]).ok();
        }
        line_start = line_end.saturating_add(1);
    }
    None
}

fn valid_frontmatter(frontmatter: &SkillFrontmatter, path: &str) -> bool {
    let valid_name = !frontmatter.name.is_empty()
        && frontmatter.name.len() <= 100
        && frontmatter
            .name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    let valid_description =
        !frontmatter.description.trim().is_empty() && frontmatter.description.len() <= 4_096;
    let parent_matches = path
        .rsplit_once('/')
        .map(|(parent, _)| parent.rsplit('/').next() == Some(frontmatter.name.as_str()))
        .unwrap_or(true);
    valid_name && valid_description && parent_matches
}

fn component_for_file(
    file: &QuarantinedFile,
    kind: ComponentKind,
    compatibility_status: CompatibilityStatus,
    confirmation_required: bool,
) -> ComponentNode {
    component_node(
        component_id(component_kind_tag(&kind), &file.path),
        kind,
        &file.path,
        file.path.rsplit('/').next().unwrap_or_default(),
        compatibility_status,
        confirmation_required,
        file.executable,
    )
}

fn component_node(
    id: String,
    kind: ComponentKind,
    source_path: &str,
    local_name: &str,
    compatibility_status: CompatibilityStatus,
    confirmation_required: bool,
    executable: bool,
) -> ComponentNode {
    ComponentNode {
        id,
        kind,
        source_path: source_path.into(),
        local_name: local_name.into(),
        compatibility_status,
        confirmation_required,
        executable,
        declared_preapproved_tools: Vec::new(),
    }
}

fn component_id(kind: &str, path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(path.as_bytes());
    format!("cmp_{:x}", hasher.finalize())
}

fn component_kind_tag(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Package => "package",
        ComponentKind::Skill => "skill",
        ComponentKind::McpServer => "mcp_server",
        ComponentKind::Executable => "executable",
        ComponentKind::Asset => "asset",
        ComponentKind::UnknownHostComponent => "unknown_host_component",
    }
}

fn finding(severity: InspectionSeverity, code: &str, path: &str) -> InspectionFinding {
    InspectionFinding {
        severity,
        code: code.into(),
        source_path: Some(path.into()),
        component_id: None,
    }
}

fn push_finding(
    findings: &mut Vec<InspectionFinding>,
    finding: InspectionFinding,
    limits: &InspectionLimits,
    phase: &str,
) -> Result<(), InspectionError> {
    ensure_collection_limit("FINDING_LIMIT", phase, limits.findings, findings.len(), 1)?;
    findings.push(finding);
    Ok(())
}

fn ensure_collection_limit(
    code: &str,
    phase: &str,
    limit: usize,
    current: usize,
    additional: usize,
) -> Result<(), InspectionError> {
    let observed = current
        .checked_add(additional)
        .ok_or_else(|| inspection_error("GRAPH_COUNT_OVERFLOW", phase, true))?;
    if observed > limit {
        return Err(limited_error(code, phase, limit as u64, observed as u64));
    }
    Ok(())
}

fn validate_source(
    source: &GithubInspectionSource,
) -> Result<GithubInspectionSource, InspectionError> {
    let safe_identity = |value: &str| {
        !value.is_empty()
            && value.len() <= 100
            && !matches!(value, "." | "..")
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    };
    if !safe_identity(&source.owner) || !safe_identity(&source.repo) {
        return Err(inspection_error("INVALID_SOURCE_IDENTITY", "source", false));
    }
    if source.commit_sha.len() != 40
        || !source
            .commit_sha
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(inspection_error(
            "SOURCE_COMMIT_NOT_IMMUTABLE",
            "source",
            false,
        ));
    }
    if !safe_source_path(&source.path) {
        return Err(inspection_error("INVALID_SOURCE_PATH", "source", false));
    }
    Ok(GithubInspectionSource {
        owner: source.owner.to_ascii_lowercase(),
        repo: source.repo.to_ascii_lowercase(),
        commit_sha: source.commit_sha.to_ascii_lowercase(),
        path: source.path.clone(),
    })
}

fn safe_source_path(path: &str) -> bool {
    path.is_empty()
        || (path.is_ascii()
            && path.len() <= InspectionLimits::default().path_bytes
            && path.split('/').count() <= InspectionLimits::default().path_depth
            && path.split('/').all(|part| {
                !part.is_empty()
                    && !matches!(part, "." | "..")
                    && !part.contains('\\')
                    && !part.contains('\0')
            }))
}

fn quarantine_archive(
    bytes: &[u8],
    source_path: &str,
    limits: &InspectionLimits,
) -> Result<QuarantinedPackage, InspectionError> {
    if bytes.len() > limits.raw_archive_bytes {
        return Err(limited_error(
            "RAW_ARCHIVE_LIMIT",
            "quarantine",
            limits.raw_archive_bytes as u64,
            bytes.len() as u64,
        ));
    }
    let entries = inspect_archive(bytes).map_err(archive_error)?;
    let outer = unique_outer_wrapper(&entries)?;
    let selected_prefix = if source_path.is_empty() {
        format!("{outer}/")
    } else {
        format!("{outer}/{source_path}/")
    };
    let selected = entries
        .iter()
        .filter_map(|entry| {
            entry
                .path
                .strip_prefix(&selected_prefix)
                .map(|path| (entry, path))
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(inspection_error(
            "SOURCE_PATH_NOT_FOUND",
            "quarantine",
            true,
        ));
    }
    validate_selected_paths(&selected, limits)?;

    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| inspection_error("INVALID_ARCHIVE", "quarantine", true))?;
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    for (entry, relative_path) in selected {
        if entry.directory {
            continue;
        }
        if files.len() >= limits.files {
            return Err(limited_error(
                "FILE_LIMIT",
                "quarantine",
                limits.files as u64,
                (files.len() + 1) as u64,
            ));
        }
        validate_entry_limits(entry, relative_path, limits)?;
        let entry_size = usize::try_from(entry.size)
            .map_err(|_| inspection_error("FILE_SIZE_OVERFLOW", "quarantine", true))?;
        total_bytes = total_bytes
            .checked_add(entry_size)
            .ok_or_else(|| inspection_error("TOTAL_SIZE_OVERFLOW", "quarantine", true))?;
        if total_bytes > limits.expanded_total_bytes {
            return Err(limited_error(
                "EXPANDED_TOTAL_LIMIT",
                "quarantine",
                limits.expanded_total_bytes as u64,
                total_bytes as u64,
            ));
        }
        let mut file = archive
            .by_index(entry.index)
            .map_err(|_| inspection_error("ARCHIVE_READ_FAILED", "quarantine", true))?;
        let mut content = Vec::with_capacity(entry_size);
        file.read_to_end(&mut content)
            .map_err(|_| inspection_error("ARCHIVE_READ_FAILED", "quarantine", true))?;
        if content.len() != entry_size {
            return Err(inspection_error(
                "ARCHIVE_SIZE_MISMATCH",
                "quarantine",
                true,
            ));
        }
        files.push(QuarantinedFile {
            path: relative_path.to_string(),
            executable: entry_is_executable(entry, relative_path, &content),
            content,
        });
    }
    let mut findings = non_ascii_path_findings(&files, limits)?;
    findings.sort();
    Ok(QuarantinedPackage {
        content_sha256: canonical_content_hash(&files),
        files,
        total_bytes,
        findings,
    })
}

fn unique_outer_wrapper(entries: &[EntryMeta]) -> Result<String, InspectionError> {
    let roots = entries
        .iter()
        .filter_map(|entry| entry.path.split('/').next())
        .collect::<BTreeSet<_>>();
    if roots.len() != 1 {
        return Err(inspection_error(
            "INVALID_GITHUB_ARCHIVE",
            "quarantine",
            true,
        ));
    }
    let outer = roots.into_iter().next().unwrap_or_default().to_string();
    let prefix = format!("{outer}/");
    if outer.is_empty()
        || !entries.iter().all(|entry| {
            (entry.path == outer && entry.directory) || entry.path.starts_with(&prefix)
        })
    {
        return Err(inspection_error(
            "INVALID_GITHUB_ARCHIVE",
            "quarantine",
            true,
        ));
    }
    Ok(outer)
}

fn validate_selected_paths(
    selected: &[(&EntryMeta, &str)],
    limits: &InspectionLimits,
) -> Result<(), InspectionError> {
    let mut folded_prefixes = BTreeMap::<String, (String, bool)>::new();
    for (entry, path) in selected {
        if is_reserved_path(path) {
            return Err(path_error("RESERVED_ARCHIVE_PATH", "quarantine", path));
        }
        let mut prefix = String::new();
        let components = path.split('/').collect::<Vec<_>>();
        for (index, component) in components.iter().enumerate() {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            let is_file = index + 1 == components.len() && !entry.directory;
            let folded = prefix.to_ascii_lowercase();
            if let Some((previous, previous_is_file)) = folded_prefixes.get(&folded) {
                if previous != &prefix {
                    return Err(path_error("PATH_IDENTITY_COLLISION", "quarantine", path));
                }
                if *previous_is_file != is_file {
                    return Err(path_error("FILE_DIRECTORY_CONFLICT", "quarantine", path));
                }
            } else {
                folded_prefixes.insert(folded, (prefix.clone(), is_file));
            }
        }
        if path.len() > limits.path_bytes {
            return Err(path_error("PATH_LIMIT", "quarantine", path));
        }
    }
    Ok(())
}

fn validate_entry_limits(
    entry: &EntryMeta,
    path: &str,
    limits: &InspectionLimits,
) -> Result<(), InspectionError> {
    if entry.size > limits.single_file_bytes as u64 {
        return Err(limited_path_error(
            "SINGLE_FILE_LIMIT",
            "quarantine",
            path,
            limits.single_file_bytes as u64,
            entry.size,
        ));
    }
    if entry.compressed_size == 0 && entry.size > 0
        || entry.size
            > entry
                .compressed_size
                .saturating_mul(limits.compression_ratio)
    {
        return Err(limited_path_error(
            "COMPRESSION_RATIO_LIMIT",
            "quarantine",
            path,
            limits.compression_ratio,
            entry.size,
        ));
    }
    if is_skill_manifest(path) && entry.size > limits.manifest_bytes as u64 {
        return Err(limited_path_error(
            "MANIFEST_LIMIT",
            "quarantine",
            path,
            limits.manifest_bytes as u64,
            entry.size,
        ));
    }
    Ok(())
}

fn is_reserved_path(path: &str) -> bool {
    path.split('/').any(|part| {
        part == IMPORT_ORIGIN_FILE || part == CATALOG_STAMP_FILE || part.starts_with(".csswitch-")
    })
}

fn entry_is_executable(entry: &EntryMeta, path: &str, content: &[u8]) -> bool {
    let mode_executable = entry.mode.map(|mode| mode & 0o111 != 0).unwrap_or(false);
    let scripted_path = path.starts_with("bin/") || path.starts_with("scripts/");
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    let executable_extension = matches!(
        extension.as_deref(),
        Some(
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "py"
                | "js"
                | "mjs"
                | "cjs"
                | "rb"
                | "pl"
                | "php"
                | "exe"
                | "bat"
                | "cmd"
                | "ps1"
        )
    );
    mode_executable || scripted_path || executable_extension || content.starts_with(b"#!")
}

fn non_ascii_path_findings(
    files: &[QuarantinedFile],
    limits: &InspectionLimits,
) -> Result<Vec<InspectionFinding>, InspectionError> {
    let mut findings = Vec::new();
    for file in files.iter().filter(|file| !file.path.is_ascii()) {
        push_finding(
            &mut findings,
            InspectionFinding {
                severity: InspectionSeverity::Blocking,
                code: "PATH_IDENTITY_UNSUPPORTED".into(),
                source_path: Some(file.path.clone()),
                component_id: None,
            },
            limits,
            "quarantine",
        )?;
    }
    Ok(findings)
}

fn canonical_content_hash(files: &[QuarantinedFile]) -> String {
    const DOMAIN_SEPARATOR: &[u8] = b"csswitch.package-inspection.content-sha256.v1\0";

    let mut ordered = files.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_SEPARATOR);
    for file in ordered {
        hasher.update((file.path.len() as u64).to_le_bytes());
        hasher.update(file.path.as_bytes());
        hasher.update([u8::from(file.executable)]);
        hasher.update((file.content.len() as u64).to_le_bytes());
        hasher.update(&file.content);
    }
    format!("{:x}", hasher.finalize())
}

fn archive_error(error: crate::InstallError) -> InspectionError {
    InspectionError {
        code: error.code,
        phase: error.phase,
        quarantined: true,
        source_path: None,
        limit: None,
        observed: None,
    }
}

fn inspection_error(code: &str, phase: &str, quarantined: bool) -> InspectionError {
    InspectionError {
        code: code.into(),
        phase: phase.into(),
        quarantined,
        source_path: None,
        limit: None,
        observed: None,
    }
}

fn path_error(code: &str, phase: &str, path: &str) -> InspectionError {
    InspectionError {
        source_path: Some(path.into()),
        ..inspection_error(code, phase, true)
    }
}

fn limited_error(code: &str, phase: &str, limit: u64, observed: u64) -> InspectionError {
    InspectionError {
        limit: Some(limit),
        observed: Some(observed),
        ..inspection_error(code, phase, true)
    }
}

fn limited_path_error(
    code: &str,
    phase: &str,
    path: &str,
    limit: u64,
    observed: u64,
) -> InspectionError {
    InspectionError {
        source_path: Some(path.into()),
        ..limited_error(code, phase, limit, observed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    fn source(path: &str) -> GithubInspectionSource {
        GithubInspectionSource {
            owner: "Example".into(),
            repo: "Skills".into(),
            commit_sha: COMMIT.into(),
            path: path.into(),
        }
    }

    fn zip_bytes(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content, mode) in entries {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .unix_permissions(*mode),
                )
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn zip_bytes_with_directories(
        directories: &[(&str, u32)],
        files: &[(&str, &[u8], u32)],
    ) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, mode) in directories {
            writer
                .add_directory(*name, SimpleFileOptions::default().unix_permissions(*mode))
                .unwrap();
        }
        for (name, content, mode) in files {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .unix_permissions(*mode),
                )
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn zip_bytes_owned(entries: impl IntoIterator<Item = (String, Vec<u8>, u32)>) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content, mode) in entries {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Deflated)
                        .unix_permissions(mode),
                )
                .unwrap();
            writer.write_all(&content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn replace_all_same_length(bytes: &mut [u8], from: &[u8], to: &[u8]) {
        assert_eq!(from.len(), to.len());
        let mut offset = 0;
        while let Some(found) = bytes[offset..]
            .windows(from.len())
            .position(|window| window == from)
        {
            let start = offset + found;
            bytes[start..start + to.len()].copy_from_slice(to);
            offset = start + to.len();
        }
    }

    fn positive_archive() -> Vec<u8> {
        zip_bytes(&[
            (
                "skills-0123456789ab/skills/demo/SKILL.md",
                include_bytes!("../tests/fixtures/inspection/positive/SKILL.md"),
                0o644,
            ),
            (
                "skills-0123456789ab/skills/demo/assets/note.txt",
                include_bytes!("../tests/fixtures/inspection/positive/note.txt"),
                0o644,
            ),
            (
                "skills-0123456789ab/skills/demo/scripts/run.sh",
                include_bytes!("../tests/fixtures/inspection/positive/scripts/run.sh"),
                0o755,
            ),
        ])
    }

    fn node(report: &InspectionReportV1, kind: ComponentKind) -> &ComponentNode {
        report
            .graph
            .nodes
            .iter()
            .find(|node| node.kind == kind)
            .unwrap()
    }

    #[test]
    fn positive_skill_builds_stable_canonical_graph_without_effects() {
        let report =
            inspect_github_skill_archive(&source("skills/demo"), &positive_archive()).unwrap();
        let serialized = serde_json::to_string(&report).unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/inspection/positive/expected.report.v1.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(&report).unwrap(), expected);
        assert_eq!(
            format!("{:x}", Sha256::digest(serialized.as_bytes())),
            include_str!("../tests/fixtures/inspection/positive/expected.report.sha256").trim()
        );
        assert_eq!(report.outcome, InspectionOutcome::Complete);
        assert_eq!(report.source_claim.verification, CALLER_ASSERTED_UNVERIFIED);
        assert_eq!(
            node(&report, ComponentKind::Skill).declared_preapproved_tools,
            ["Read", "Search"]
        );
        assert!(node(&report, ComponentKind::Executable).confirmation_required);
        assert_eq!(report.effects.apply, "not_run");
        assert_eq!(report.effects.install, "not_run");
        assert_eq!(report.effects.science_attach, "not_run");
        assert_eq!(report.effects.mcp_launch, "not_run");
        let plan = crate::build_skill_plan(&report).unwrap();
        assert_eq!(plan.eligibility, crate::PlanEligibility::InspectOnly);
        assert!(plan
            .effects
            .iter()
            .all(|effect| effect.apply == crate::EffectApplyStateV1::NotRun));
    }

    #[test]
    fn malformed_manifests_yield_stable_blocking_findings() {
        for manifest in [
            b"# missing frontmatter\n".as_slice(),
            b"---\nname: demo\ndescription: valid\nunknown: value\n---\n".as_slice(),
            b"---\nname: [demo]\ndescription: valid\n---\n".as_slice(),
            b"---\nname: demo\ndescription: [unterminated\n---\n".as_slice(),
        ] {
            let archive = zip_bytes(&[("repo/skills/demo/SKILL.md", manifest, 0o644)]);
            let report = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap();
            assert_eq!(report.outcome, InspectionOutcome::Partial);
            assert!(report.findings.iter().any(|finding| {
                finding.code == "SKILL_FRONTMATTER_UNSUPPORTED"
                    && finding.severity == InspectionSeverity::Blocking
            }));
            assert_eq!(
                node(&report, ComponentKind::Skill).compatibility_status,
                CompatibilityStatus::Unsupported
            );
        }
        let allowed_tools = (0..129)
            .map(|index| format!("tool{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let manifest =
            format!("---\nname: demo\ndescription: valid\nallowed-tools: {allowed_tools}\n---\n");
        let archive = zip_bytes(&[("repo/skills/demo/SKILL.md", manifest.as_bytes(), 0o644)]);

        let report = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap();
        assert_eq!(report.outcome, InspectionOutcome::Partial);
        assert_eq!(
            node(&report, ComponentKind::Skill).compatibility_status,
            CompatibilityStatus::Unsupported
        );
        assert!(report.findings.iter().any(|finding| {
            finding.code == "SKILL_ALLOWED_TOOLS_UNSUPPORTED"
                && finding.severity == InspectionSeverity::Blocking
        }));
    }

    #[test]
    fn source_path_special_entry_collision_and_limits_fail_closed() {
        for source in [source("skills//demo"), source("../skills")]
            .into_iter()
            .chain([GithubInspectionSource {
                commit_sha: "not-a-commit".into(),
                ..source("skills/demo")
            }])
        {
            assert!(
                !inspect_github_skill_archive(&source, &positive_archive())
                    .unwrap_err()
                    .quarantined
            );
        }

        let valid = include_bytes!("../tests/fixtures/inspection/positive/SKILL.md");
        let archive = zip_bytes_with_directories(
            &[
                ("repo/", 0o755),
                ("repo/skills/", 0o755),
                ("repo/skills/demo/", 0o755),
                ("repo/skills/demo/assets/", 0o755),
            ],
            &[
                ("repo/skills/demo/SKILL.md", valid, 0o644),
                ("repo/skills/demo/assets/note.txt", b"retained", 0o644),
            ],
        );
        let report = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap();
        assert_eq!(report.package.file_count, 2);

        let archive = zip_bytes_with_directories(
            &[("repo/skills/demo/assets/", 0o755)],
            &[
                ("repo/skills/demo/SKILL.md", valid, 0o644),
                ("repo/skills/demo/ASSETS/note.txt", b"aliased", 0o644),
            ],
        );
        let error = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap_err();
        assert_eq!(error.code, "PATH_IDENTITY_COLLISION");
        assert!(error.quarantined);

        let archive = zip_bytes(&[
            ("repo/skills/demo/SKILL.md", valid, 0o644),
            ("repo/skills/demo/assets/a.txt", b"first", 0o644),
            ("repo/skills/demo/ASSETS/b.txt", b"second", 0o644),
        ]);
        let error = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap_err();
        assert_eq!(error.code, "PATH_IDENTITY_COLLISION");
        assert!(error.quarantined);

        let archive = zip_bytes(&[
            ("repo/skills/demo/SKILL.md", valid, 0o644),
            ("repo/skills/demo/assets", b"not a directory", 0o644),
            ("repo/skills/demo/assets/note.txt", b"nested", 0o644),
        ]);
        let error = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap_err();
        assert_eq!(error.code, "FILE_DIRECTORY_CONFLICT");
        assert!(error.quarantined);

        let archive = zip_bytes(&[
            ("repo", b"not a directory", 0o644),
            ("repo/skills/demo/SKILL.md", valid, 0o644),
        ]);
        let error = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap_err();
        assert_eq!(error.code, "INVALID_GITHUB_ARCHIVE");
        assert!(error.quarantined);

        let nested_manifest = |length| {
            let mut manifest = b"---\nname: nested\ndescription: valid\n---\n".to_vec();
            let mut state = 0x6d2b_79f5_u32;
            while manifest.len() < length {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                manifest.push((state >> 24) as u8);
            }
            manifest
        };
        let manifest_limit = InspectionLimits::default().manifest_bytes;
        let archive = zip_bytes(&[(
            "repo/skills/nested/SKILL.md",
            &nested_manifest(manifest_limit),
            0o644,
        )]);
        assert!(inspect_github_skill_archive(&source("skills/nested"), &archive).is_ok());

        let archive = zip_bytes(&[(
            "repo/skills/nested/SKILL.md",
            &nested_manifest(manifest_limit + 1),
            0o644,
        )]);
        let error = inspect_github_skill_archive(&source("skills/nested"), &archive).unwrap_err();
        assert_eq!(error.code, "MANIFEST_LIMIT");
        assert_eq!(error.source_path.as_deref(), Some("SKILL.md"));
        assert_eq!(error.limit, Some(manifest_limit as u64));
        assert_eq!(error.observed, Some((manifest_limit + 1) as u64));

        for (archive, expected_code) in [
            (
                zip_bytes(&[
                    ("repo/skills/demo/SKILL.md", valid, 0o644),
                    ("repo/skills/demo/skill.md", b"asset", 0o644),
                ]),
                "PATH_IDENTITY_COLLISION",
            ),
            (
                zip_bytes(&[
                    ("repo/skills/demo/SKILL.md", valid, 0o644),
                    ("repo/skills/demo/.import-origin", b"reserved", 0o644),
                ]),
                "RESERVED_ARCHIVE_PATH",
            ),
            (
                zip_bytes(&[
                    ("repo/skills/demo/SKILL.md", valid, 0o644),
                    (
                        "repo/skills/demo/large.bin",
                        &vec![b'x'; 4 * 1024 * 1024 + 1],
                        0o644,
                    ),
                ]),
                "SINGLE_FILE_LIMIT",
            ),
            (
                zip_bytes(&[
                    ("repo/skills/demo/SKILL.md", valid, 0o644),
                    ("repo/skills/demo/ratio.txt", &vec![b'x'; 512 * 1024], 0o644),
                ]),
                "COMPRESSION_RATIO_LIMIT",
            ),
        ] {
            let error = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap_err();
            assert_eq!(error.code, expected_code);
            assert!(error.quarantined);
        }

        let mut traversal = zip_bytes(&[("repo/skills/demo/xx/SKILL.md", valid, 0o644)]);
        replace_all_same_length(&mut traversal, b"/xx/", b"/../");
        let error = inspect_github_skill_archive(&source("skills/demo"), &traversal).unwrap_err();
        assert_eq!(error.code, "UNSAFE_ARCHIVE_PATH");
        assert!(error.quarantined);

        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "repo/skills/demo/SKILL.md",
                SimpleFileOptions::default().unix_permissions(0o644),
            )
            .unwrap();
        writer.write_all(valid).unwrap();
        writer
            .add_symlink(
                "repo/skills/demo/scripts/current",
                "../outside",
                SimpleFileOptions::default(),
            )
            .unwrap();
        let special = writer.finish().unwrap().into_inner();
        let error = inspect_github_skill_archive(&source("skills/demo"), &special).unwrap_err();
        assert_eq!(error.code, "UNSUPPORTED_ARCHIVE_ENTRY");
        assert!(error.quarantined);

        let mut duplicate = zip_bytes(&[
            ("repo/skills/demo/SKILL.md", valid, 0o644),
            ("repo/skills/demo/OTHER.md", b"duplicate", 0o644),
        ]);
        replace_all_same_length(&mut duplicate, b"OTHER.md", b"SKILL.md");
        let error = inspect_github_skill_archive(&source("skills/demo"), &duplicate).unwrap_err();
        assert_eq!(error.code, "DUPLICATE_ARCHIVE_PATH");
        assert!(error.quarantined);

        let archive = zip_bytes(&[
            ("repo/skills/demo/SKILL.md", valid, 0o644),
            ("repo/skills/demo/assets/你好.txt", b"retained", 0o644),
        ]);
        let report = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap();
        assert_eq!(report.outcome, InspectionOutcome::Partial);
        assert!(
            report.package.file_count == 2
                && report
                    .graph
                    .nodes
                    .iter()
                    .any(|node| node.source_path == "assets/你好.txt")
        );
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "PATH_IDENTITY_UNSUPPORTED"));

        let archive = zip_bytes(&[
            ("repo/skills/demo/SKILL.md", valid, 0o644),
            ("repo/skills/demo/assets/你好.txt", b"retained", 0o644),
            ("repo/skills/demo/manifest.json", b"{}", 0o644),
            ("repo/skills/demo/plugin.json", b"{}", 0o644),
        ]);
        let error = inspect_github_skill_archive_with_limits(
            &source("skills/demo"),
            &archive,
            InspectionLimits {
                findings: 2,
                ..InspectionLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "FINDING_LIMIT");
        assert_eq!(error.phase, "graph");
        assert_eq!(error.limit, Some(2));
        assert_eq!(error.observed, Some(3));

        let archive = zip_bytes(&[
            ("repo/skills/demo/SKILL.md", valid, 0o644),
            ("repo/skills/demo/note.txt", b"asset", 0o644),
        ]);
        for (limits, expected_code, expected_limit, expected_observed) in [
            (
                InspectionLimits {
                    graph_nodes: 2,
                    graph_edges: 16,
                    ..InspectionLimits::default()
                },
                "GRAPH_NODE_LIMIT",
                2,
                3,
            ),
            (
                InspectionLimits {
                    graph_nodes: 16,
                    graph_edges: 1,
                    ..InspectionLimits::default()
                },
                "GRAPH_EDGE_LIMIT",
                1,
                2,
            ),
        ] {
            let error =
                inspect_github_skill_archive_with_limits(&source("skills/demo"), &archive, limits)
                    .unwrap_err();
            assert_eq!(error.code, expected_code);
            assert_eq!(error.phase, "graph");
            assert_eq!(error.limit, Some(expected_limit));
            assert_eq!(error.observed, Some(expected_observed));
        }
    }

    #[test]
    fn mcp_report_is_blocked_and_redacts_command_env_headers_and_url() {
        let secret = "inspection-secret-sentinel";
        let archive = zip_bytes(&[
            (
                "repo/skills/demo/SKILL.md",
                include_bytes!("../tests/fixtures/inspection/positive/SKILL.md"),
                0o644,
            ),
            (
                "repo/skills/demo/redaction.mcp.json",
                include_bytes!("../tests/fixtures/inspection/negative/redaction.mcp.json"),
                0o644,
            ),
        ]);
        let report = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap();
        assert_eq!(
            node(&report, ComponentKind::McpServer).compatibility_status,
            CompatibilityStatus::Unsupported
        );
        assert!(node(&report, ComponentKind::McpServer).confirmation_required);
        assert!(!serde_json::to_string(&report).unwrap().contains(secret));

        let mut entries = vec![(
            "repo/skills/demo/SKILL.md".to_string(),
            include_bytes!("../tests/fixtures/inspection/positive/SKILL.md").to_vec(),
            0o644,
        )];
        entries.extend((0..129).map(|index| {
            (
                format!("repo/skills/demo/server-{index}.mcp.json"),
                b"{}".to_vec(),
                0o644,
            )
        }));
        let error = inspect_github_skill_archive(&source("skills/demo"), &zip_bytes_owned(entries))
            .unwrap_err();
        assert_eq!(error.code, "MCP_MANIFEST_LIMIT");
        assert_eq!(error.phase, "graph");
        assert_eq!(error.limit, Some(128));
        assert_eq!(error.observed, Some(129));
    }

    #[test]
    fn vendor_components_remain_unsupported_and_never_become_assets() {
        let archive = zip_bytes(&[
            (
                "repo/skills/demo/SKILL.md",
                include_bytes!("../tests/fixtures/inspection/positive/SKILL.md"),
                0o644,
            ),
            (
                "repo/skills/demo/.codex-plugin/plugin.json",
                include_bytes!("../tests/fixtures/inspection/negative/plugin.json"),
                0o644,
            ),
        ]);
        let report = inspect_github_skill_archive(&source("skills/demo"), &archive).unwrap();
        let plugin = report
            .graph
            .nodes
            .iter()
            .find(|node| node.source_path.ends_with("plugin.json"))
            .unwrap();
        assert_eq!(plugin.kind, ComponentKind::UnknownHostComponent);
        assert_eq!(
            plugin.compatibility_status,
            CompatibilityStatus::Unsupported
        );
        assert!(plugin.confirmation_required);
    }

    #[test]
    fn report_has_no_apply_conversion_or_install_identity() {
        let first =
            inspect_github_skill_archive(&source("skills/demo"), &positive_archive()).unwrap();
        let second =
            inspect_github_skill_archive(&source("skills/demo"), &positive_archive()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.graph.sha256, second.graph.sha256);

        let nul_delimited = vec![QuarantinedFile {
            path: "a".into(),
            content: b"x\0b\0y".to_vec(),
            executable: false,
        }];
        let separate_files = vec![
            QuarantinedFile {
                path: "a".into(),
                content: b"x".to_vec(),
                executable: false,
            },
            QuarantinedFile {
                path: "b".into(),
                content: b"y".to_vec(),
                executable: false,
            },
        ];
        assert_ne!(
            canonical_content_hash(&nul_delimited),
            canonical_content_hash(&separate_files)
        );

        let non_executable = QuarantinedFile {
            path: "same".into(),
            content: b"content".to_vec(),
            executable: false,
        };
        let executable = QuarantinedFile {
            executable: true,
            ..non_executable.clone()
        };
        assert_ne!(
            canonical_content_hash(&[non_executable]),
            canonical_content_hash(&[executable])
        );

        let reverse_order = vec![
            QuarantinedFile {
                path: "z".into(),
                content: b"last".to_vec(),
                executable: true,
            },
            QuarantinedFile {
                path: "a".into(),
                content: b"first".to_vec(),
                executable: false,
            },
        ];
        let sorted_order = reverse_order.iter().cloned().rev().collect::<Vec<_>>();
        assert_eq!(
            canonical_content_hash(&reverse_order),
            canonical_content_hash(&sorted_order)
        );

        let serialized = serde_json::to_string(&first).unwrap();
        for forbidden in ["receipt", "resolved_commit", "install_id", "apply_digest"] {
            assert!(!serialized.contains(forbidden));
        }
    }
}
