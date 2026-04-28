//! Local host artifacts for Core Intelligence external outputs.
//!
//! This module turns the existing read-only payload-provider commands into the
//! first reusable trusted local host artifact: a browser-openable snippet bundle
//! written under `app_root/integrations/core-intelligence/browser-snippet-v1/`.
//! The bundle stays local-first and manual-reviewable: the renderer can preview
//! file contents before writing them, and the generated HTML never fetches over
//! HTTP from `file://`.

use super::{
    get_intelligence_embed_cards, get_intelligence_public_snapshot,
    get_intelligence_widget_snapshot,
};
use crate::{
    config::{ProjectPaths, ensure_paths},
    models::{
        AppConfig, GeneratedFile, IntelligenceEmbedCardsRequest, IntelligenceInstalledLocalHost,
        IntelligenceLocalHostBuildResult, IntelligenceLocalHostBundle,
        IntelligenceLocalHostPreview, IntelligenceLocalHostRequest, IntelligencePublicSnapshot,
        IntelligenceWidgetSnapshot, ScopedDateRangeRequest,
    },
    utils::now_rfc3339,
};
use anyhow::{Context, Result};
use std::{collections::BTreeSet, fs, path::PathBuf};

#[cfg(test)]
use std::path::Path;

const BROWSER_SNIPPET_HOST_ID: &str = "browser-snippet-v1";
const LOCAL_HOST_BUNDLE_VERSION: &str = "pathkeep.core-intelligence.local-host.v1";
const EMBED_CARD_LIMIT: u32 = 6;
const WIDGET_CARD_LIMIT: u32 = 4;

pub fn preview_intelligence_local_host(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostPreview> {
    let bundle = build_local_host_bundle(paths, config, key, request)?;
    let texts = host_copy(&request.locale);
    let artifact_root = local_host_root(paths);
    let entry_file_path = artifact_root.join("index.html");
    let bundle_file_path = artifact_root.join("bundle.json");
    let html = render_browser_snippet_html(&bundle, &texts);
    let bundle_json =
        serde_json::to_string_pretty(&bundle).context("serializing local host bundle")?;
    let generated_files = vec![
        GeneratedFile {
            relative_path: host_relative_path("index.html"),
            absolute_path: Some(entry_file_path.display().to_string()),
            purpose: texts.entry_file_purpose.to_string(),
            contents: html,
        },
        GeneratedFile {
            relative_path: host_relative_path("bundle.json"),
            absolute_path: Some(bundle_file_path.display().to_string()),
            purpose: texts.bundle_file_purpose.to_string(),
            contents: bundle_json,
        },
    ];
    let manual_steps = vec![
        texts.manual_step_review.to_string(),
        texts.manual_step_open.to_string(),
        texts.manual_step_rebuild.to_string(),
    ];
    let mut warnings = Vec::new();
    if bundle.trusted_only_card_count > 0 {
        warnings.push(texts.trusted_only_warning.to_string());
    }
    let (installed_host, installed_warnings) = load_installed_host(paths, &request.locale)?;
    warnings.extend(installed_warnings);

    Ok(IntelligenceLocalHostPreview {
        artifact_root: artifact_root.display().to_string(),
        entry_file_path: entry_file_path.display().to_string(),
        generated_files,
        bundle: bundle.clone(),
        boundary_notes: bundle.boundary_notes.clone(),
        manual_steps,
        warnings,
        installed_host,
    })
}

pub fn build_intelligence_local_host(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostBuildResult> {
    let preview = preview_intelligence_local_host(paths, config, key, request)?;
    ensure_paths(paths)?;
    let artifact_root = local_host_root(paths);
    fs::create_dir_all(&artifact_root)
        .with_context(|| format!("creating {}", artifact_root.display()))?;
    for file in &preview.generated_files {
        let absolute_path = file
            .absolute_path
            .as_deref()
            .context("local host generated file missing absolute path")?;
        fs::write(absolute_path, &file.contents)
            .with_context(|| format!("writing {absolute_path}"))?;
    }

    Ok(IntelligenceLocalHostBuildResult {
        artifact_root: preview.artifact_root.clone(),
        entry_file_path: preview.entry_file_path.clone(),
        generated_files: preview.generated_files.clone(),
        bundle: preview.bundle.clone(),
        boundary_notes: preview.boundary_notes.clone(),
        manual_steps: preview.manual_steps.clone(),
        warnings: preview.warnings.clone(),
        installed_host: Some(IntelligenceInstalledLocalHost {
            artifact_root: preview.artifact_root,
            entry_file_path: preview.entry_file_path,
            bundle: preview.bundle,
        }),
    })
}

fn build_local_host_bundle(
    paths: &ProjectPaths,
    config: &AppConfig,
    key: Option<&str>,
    request: &IntelligenceLocalHostRequest,
) -> Result<IntelligenceLocalHostBundle> {
    let embed_cards = get_intelligence_embed_cards(
        paths,
        config,
        key,
        &IntelligenceEmbedCardsRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(EMBED_CARD_LIMIT),
        },
    )?;
    let widget_snapshot = get_intelligence_widget_snapshot(
        paths,
        config,
        key,
        &IntelligenceEmbedCardsRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
            limit: Some(WIDGET_CARD_LIMIT),
        },
    )?;
    let public_snapshot = get_intelligence_public_snapshot(
        paths,
        config,
        key,
        &ScopedDateRangeRequest {
            date_range: request.date_range.clone(),
            profile_id: request.profile_id.clone(),
        },
    )?;
    let trusted_only_card_ids = trusted_only_card_ids(&embed_cards, &widget_snapshot);
    let boundary_notes = build_boundary_notes(&request.locale);

    Ok(IntelligenceLocalHostBundle {
        bundle_version: LOCAL_HOST_BUNDLE_VERSION.to_string(),
        host_id: BROWSER_SNIPPET_HOST_ID.to_string(),
        generated_at: now_rfc3339(),
        locale: request.locale.clone(),
        date_range: request.date_range.clone(),
        profile_id: request.profile_id.clone(),
        embed_cards,
        widget_snapshot,
        public_snapshot,
        trusted_only_card_count: trusted_only_card_ids.len(),
        trusted_only_card_ids,
        boundary_notes,
    })
}

fn trusted_only_card_ids(
    embed_cards: &[crate::models::IntelligenceEmbedCardPayload],
    widget_snapshot: &IntelligenceWidgetSnapshot,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for card in embed_cards {
        if card.internal_only {
            ids.insert(card.card_id.clone());
        }
    }
    for card in &widget_snapshot.highlights {
        if card.internal_only {
            ids.insert(card.card_id.clone());
        }
    }
    ids.into_iter().collect()
}

fn load_installed_host(
    paths: &ProjectPaths,
    locale: &str,
) -> Result<(Option<IntelligenceInstalledLocalHost>, Vec<String>)> {
    let artifact_root = local_host_root(paths);
    let entry_file_path = artifact_root.join("index.html");
    let bundle_path = artifact_root.join("bundle.json");
    if !bundle_path.exists() {
        return Ok((None, Vec::new()));
    }

    let texts = host_copy(locale);
    let bundle_json = match fs::read_to_string(&bundle_path) {
        Ok(bundle_json) => bundle_json,
        Err(_) => return Ok((None, vec![texts.installed_host_read_failed.to_string()])),
    };
    let bundle = match serde_json::from_str::<IntelligenceLocalHostBundle>(&bundle_json) {
        Ok(bundle) => bundle,
        Err(_) => return Ok((None, vec![texts.installed_host_parse_failed.to_string()])),
    };
    if !entry_file_path.exists() {
        return Ok((None, vec![texts.installed_host_entry_missing.to_string()]));
    }
    Ok((
        Some(IntelligenceInstalledLocalHost {
            artifact_root: artifact_root.display().to_string(),
            entry_file_path: entry_file_path.display().to_string(),
            bundle,
        }),
        Vec::new(),
    ))
}

fn local_host_root(paths: &ProjectPaths) -> PathBuf {
    paths.app_root.join("integrations").join("core-intelligence").join(BROWSER_SNIPPET_HOST_ID)
}

fn host_relative_path(file_name: &str) -> String {
    format!("integrations/core-intelligence/{BROWSER_SNIPPET_HOST_ID}/{file_name}")
}

struct HostCopy {
    app_title: &'static str,
    summary_title: &'static str,
    scope_label: &'static str,
    scope_archive_wide: &'static str,
    scope_profile_prefix: &'static str,
    window_label: &'static str,
    generated_at_label: &'static str,
    trusted_only_title: &'static str,
    trusted_only_body: &'static str,
    boundary_title: &'static str,
    embed_section_title: &'static str,
    widget_section_title: &'static str,
    public_section_title: &'static str,
    digest_section_title: &'static str,
    top_domains_title: &'static str,
    search_engines_title: &'static str,
    discovery_trend_title: &'static str,
    notes_title: &'static str,
    no_cards: &'static str,
    no_search_engines: &'static str,
    no_discovery_trend: &'static str,
    metric_visits: &'static str,
    metric_searches: &'static str,
    metric_new_sites: &'static str,
    metric_deep_read: &'static str,
    metric_refind: &'static str,
    entry_file_purpose: &'static str,
    bundle_file_purpose: &'static str,
    manual_step_review: &'static str,
    manual_step_open: &'static str,
    manual_step_rebuild: &'static str,
    trusted_only_warning: &'static str,
    installed_host_read_failed: &'static str,
    installed_host_parse_failed: &'static str,
    installed_host_entry_missing: &'static str,
}

fn host_copy(locale: &str) -> HostCopy {
    let normalized = locale.to_ascii_lowercase();
    if normalized.starts_with("zh-tw") {
        HostCopy {
            app_title: "PathKeep Core Intelligence 片段",
            summary_title: "受信任的本地宿主預覽",
            scope_label: "範圍",
            scope_archive_wide: "整個 archive",
            scope_profile_prefix: "Profile",
            window_label: "時間視窗",
            generated_at_label: "產生時間",
            trusted_only_title: "包含僅限受信任宿主的卡片",
            trusted_only_body: "這個本地片段仍包含標記為 trusted-only 的卡片。請只在你信任的本地 PathKeep 控制宿主裡打開它。",
            boundary_title: "邊界說明",
            embed_section_title: "Embed cards",
            widget_section_title: "Widget snapshot",
            public_section_title: "Public snapshot",
            digest_section_title: "摘要",
            top_domains_title: "Top domains",
            search_engines_title: "搜尋引擎",
            discovery_trend_title: "發現趨勢",
            notes_title: "備註",
            no_cards: "這個範圍裡目前沒有可用的卡片。",
            no_search_engines: "這個時間視窗裡沒有可用的搜尋引擎活動。",
            no_discovery_trend: "這個時間視窗裡沒有可用的發現趨勢點。",
            metric_visits: "Visits",
            metric_searches: "Searches",
            metric_new_sites: "New sites",
            metric_deep_read: "Deep read",
            metric_refind: "Refind",
            entry_file_purpose: "可直接在本機瀏覽器開啟的 Core Intelligence 片段。",
            bundle_file_purpose: "同一份本地宿主資料的機器可讀 JSON bundle。",
            manual_step_review: "先檢查 index.html 與 bundle.json，再把這個資料夾交給其他受信任的本地工具。",
            manual_step_open: "從這個資料夾直接打開 index.html，在受信任的本地瀏覽器宿主裡檢視它。",
            manual_step_rebuild: "只要 scope、時間視窗或語言改變，就重新建立這個本地片段。",
            trusted_only_warning: "這個本地片段包含 trusted-only 卡片，不能把它當成公開匯出。",
            installed_host_read_failed: "已安裝的本地片段存在，但目前無法讀取 bundle.json。請重新建立它。",
            installed_host_parse_failed: "已安裝的本地片段 bundle.json 無法解析。請重新建立它。",
            installed_host_entry_missing: "已安裝的本地片段缺少 index.html。請重新建立它。",
        }
    } else if normalized.starts_with("zh-cn") {
        HostCopy {
            app_title: "PathKeep Core Intelligence 片段",
            summary_title: "受信任的本地宿主预览",
            scope_label: "范围",
            scope_archive_wide: "整个 archive",
            scope_profile_prefix: "Profile",
            window_label: "时间窗口",
            generated_at_label: "生成时间",
            trusted_only_title: "包含仅限受信任宿主的卡片",
            trusted_only_body: "这个本地片段仍包含标记为 trusted-only 的卡片。请只在你信任的本地 PathKeep 控制宿主里打开它。",
            boundary_title: "边界说明",
            embed_section_title: "Embed cards",
            widget_section_title: "Widget snapshot",
            public_section_title: "Public snapshot",
            digest_section_title: "摘要",
            top_domains_title: "Top domains",
            search_engines_title: "搜索引擎",
            discovery_trend_title: "发现趋势",
            notes_title: "备注",
            no_cards: "这个范围里暂时没有可用的卡片。",
            no_search_engines: "这个时间窗口里没有可用的搜索引擎活动。",
            no_discovery_trend: "这个时间窗口里没有可用的发现趋势点。",
            metric_visits: "Visits",
            metric_searches: "Searches",
            metric_new_sites: "New sites",
            metric_deep_read: "Deep read",
            metric_refind: "Refind",
            entry_file_purpose: "可直接在本机浏览器打开的 Core Intelligence 片段。",
            bundle_file_purpose: "同一份本地宿主数据的机器可读 JSON bundle。",
            manual_step_review: "先检查 index.html 和 bundle.json，再把这个文件夹交给其他受信任的本地工具。",
            manual_step_open: "直接从这个文件夹打开 index.html，在受信任的本地浏览器宿主里查看它。",
            manual_step_rebuild: "只要 scope、时间窗口或语言发生变化，就重新创建这个本地片段。",
            trusted_only_warning: "这个本地片段包含 trusted-only 卡片，不能把它当成公开导出。",
            installed_host_read_failed: "已安装的本地片段存在，但暂时无法读取 bundle.json。请重新创建它。",
            installed_host_parse_failed: "已安装的本地片段 bundle.json 无法解析。请重新创建它。",
            installed_host_entry_missing: "已安装的本地片段缺少 index.html。请重新创建它。",
        }
    } else {
        HostCopy {
            app_title: "PathKeep Core Intelligence Snippet",
            summary_title: "Trusted local host preview",
            scope_label: "Scope",
            scope_archive_wide: "Archive-wide",
            scope_profile_prefix: "Profile",
            window_label: "Window",
            generated_at_label: "Generated at",
            trusted_only_title: "Trusted-only cards are still present",
            trusted_only_body: "This local snippet still includes cards marked trusted-only. Keep it inside a trusted PathKeep-controlled local host.",
            boundary_title: "Boundary notes",
            embed_section_title: "Embed cards",
            widget_section_title: "Widget snapshot",
            public_section_title: "Public snapshot",
            digest_section_title: "Digest",
            top_domains_title: "Top domains",
            search_engines_title: "Search engines",
            discovery_trend_title: "Discovery trend",
            notes_title: "Notes",
            no_cards: "No cards are available for this scope yet.",
            no_search_engines: "No search-engine activity is available in this window.",
            no_discovery_trend: "No discovery-trend points are available in this window.",
            metric_visits: "Visits",
            metric_searches: "Searches",
            metric_new_sites: "New sites",
            metric_deep_read: "Deep read",
            metric_refind: "Refind",
            entry_file_purpose: "Core Intelligence snippet that can be opened directly in a local browser.",
            bundle_file_purpose: "Machine-readable JSON bundle for the same local host artifact.",
            manual_step_review: "Review index.html and bundle.json before handing this folder to another trusted local tool.",
            manual_step_open: "Open index.html from this folder inside a trusted local browser surface.",
            manual_step_rebuild: "Rebuild this local snippet whenever scope, window, or locale changes.",
            trusted_only_warning: "This local snippet includes trusted-only cards and should not be treated like a public export.",
            installed_host_read_failed: "An installed local host exists, but bundle.json could not be read. Rebuild it to restore verify state.",
            installed_host_parse_failed: "An installed local host bundle exists, but bundle.json could not be parsed. Rebuild it to restore verify state.",
            installed_host_entry_missing: "An installed local host bundle exists, but index.html is missing. Rebuild it to restore verify state.",
        }
    }
}

fn build_boundary_notes(locale: &str) -> Vec<String> {
    let normalized = locale.to_ascii_lowercase();
    if normalized.starts_with("zh-tw") {
        vec![
            "這個本地宿主只使用 deterministic Core Intelligence read models。".to_string(),
            "trusted-only 卡片必須留在 PathKeep 控制的受信任本地 surface 裡。".to_string(),
            "public snapshot 仍會保持去識別化，不包含 visit-level URL 或識別欄位。".to_string(),
        ]
    } else if normalized.starts_with("zh-cn") {
        vec![
            "这个本地宿主只使用 deterministic Core Intelligence read models。".to_string(),
            "trusted-only 卡片必须留在 PathKeep 控制的受信任本地 surface 里。".to_string(),
            "public snapshot 仍会保持脱敏，不包含 visit-level URL 或标识字段。".to_string(),
        ]
    } else {
        vec![
            "This local host only uses deterministic Core Intelligence read models.".to_string(),
            "Trusted-only cards must stay inside PathKeep-controlled local surfaces.".to_string(),
            "The public snapshot stays redacted and omits visit-level URLs or identifiers."
                .to_string(),
        ]
    }
}

fn render_browser_snippet_html(bundle: &IntelligenceLocalHostBundle, texts: &HostCopy) -> String {
    let scope_value = bundle
        .profile_id
        .as_deref()
        .map(|profile_id| format!("{}: {}", texts.scope_profile_prefix, escape_html(profile_id)))
        .unwrap_or_else(|| texts.scope_archive_wide.to_string());
    let trusted_only = if bundle.trusted_only_card_count > 0 {
        format!(
            r#"<section class="callout callout--warning"><strong>{}</strong><p>{}</p></section>"#,
            escape_html(texts.trusted_only_title),
            escape_html(texts.trusted_only_body)
        )
    } else {
        String::new()
    };

    format!(
        r#"<!doctype html>
<html lang="{lang}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <style>
      :root {{
        color-scheme: light dark;
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        background: #0f172a;
        color: #e2e8f0;
      }}
      body {{
        margin: 0;
        background: linear-gradient(180deg, #0f172a 0%, #111827 100%);
        color: #e2e8f0;
      }}
      main {{
        max-width: 1080px;
        margin: 0 auto;
        padding: 32px 20px 48px;
        display: grid;
        gap: 24px;
      }}
      section, article {{
        border: 1px solid rgba(148, 163, 184, 0.25);
        background: rgba(15, 23, 42, 0.72);
        border-radius: 16px;
        padding: 20px;
      }}
      h1, h2, h3, p {{
        margin: 0;
      }}
      .hero {{
        display: grid;
        gap: 12px;
      }}
      .hero p {{
        color: #cbd5e1;
      }}
      .meta-grid, .digest-grid, .card-grid, .stats-grid {{
        display: grid;
        gap: 12px;
      }}
      .meta-grid, .digest-grid {{
        grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
      }}
      .card-grid {{
        grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      }}
      .stats-grid {{
        grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      }}
      .label {{
        display: block;
        color: #94a3b8;
        font-size: 13px;
        margin-bottom: 6px;
      }}
      .value {{
        font-weight: 600;
        overflow-wrap: anywhere;
      }}
      .callout {{
        display: grid;
        gap: 8px;
      }}
      .callout--warning {{
        border-color: rgba(250, 204, 21, 0.55);
        background: rgba(120, 53, 15, 0.35);
      }}
      .note-list, .simple-list {{
        display: grid;
        gap: 8px;
      }}
      .simple-list li, .note-list li {{
        color: #cbd5e1;
      }}
      ul {{
        margin: 0;
        padding-left: 18px;
      }}
      .card {{
        display: grid;
        gap: 10px;
      }}
      .eyebrow {{
        color: #7dd3fc;
        font-size: 12px;
        text-transform: uppercase;
        letter-spacing: 0.08em;
      }}
      .badge {{
        display: inline-flex;
        align-items: center;
        padding: 4px 8px;
        border-radius: 999px;
        background: rgba(250, 204, 21, 0.15);
        color: #fde68a;
        font-size: 12px;
        font-weight: 600;
      }}
      .metric {{
        color: #cbd5e1;
        font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
        font-size: 13px;
      }}
      .subgrid {{
        display: grid;
        gap: 16px;
      }}
      .table-like {{
        display: grid;
        gap: 8px;
      }}
      .row {{
        display: flex;
        justify-content: space-between;
        gap: 12px;
        color: #cbd5e1;
      }}
      .row span:last-child {{
        font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      }}
    </style>
  </head>
  <body>
    <main>
      <section class="hero">
        <h1>{title}</h1>
        <p>{summary_title}</p>
      </section>

      <section class="meta-grid">
        <div>
          <span class="label">{scope_label}</span>
          <span class="value">{scope_value}</span>
        </div>
        <div>
          <span class="label">{window_label}</span>
          <span class="value">{window_value}</span>
        </div>
        <div>
          <span class="label">{generated_at_label}</span>
          <span class="value">{generated_at}</span>
        </div>
      </section>

      {trusted_only}

      <section class="subgrid">
        <h2>{boundary_title}</h2>
        <ul class="note-list">
          {boundary_notes}
        </ul>
      </section>

      <section class="subgrid">
        <h2>{digest_title}</h2>
        {digest_summary}
      </section>

      <section class="subgrid">
        <h2>{embed_title}</h2>
        {embed_cards}
      </section>

      <section class="subgrid">
        <h2>{widget_title}</h2>
        {widget_summary}
      </section>

      <section class="subgrid">
        <h2>{public_title}</h2>
        {public_summary}
      </section>
    </main>
  </body>
</html>
"#,
        lang = escape_html(&bundle.locale),
        title = escape_html(texts.app_title),
        summary_title = escape_html(texts.summary_title),
        scope_label = escape_html(texts.scope_label),
        scope_value = scope_value,
        window_label = escape_html(texts.window_label),
        window_value =
            escape_html(&format!("{} → {}", bundle.date_range.start, bundle.date_range.end)),
        generated_at_label = escape_html(texts.generated_at_label),
        generated_at = escape_html(&bundle.generated_at),
        trusted_only = trusted_only,
        boundary_title = escape_html(texts.boundary_title),
        boundary_notes = render_notes(&bundle.boundary_notes),
        digest_title = escape_html(texts.digest_section_title),
        digest_summary = render_digest_summary(&bundle.widget_snapshot, texts),
        embed_title = escape_html(texts.embed_section_title),
        embed_cards = render_embed_cards(&bundle.embed_cards, texts),
        widget_title = escape_html(texts.widget_section_title),
        widget_summary = render_widget_summary(&bundle.widget_snapshot, texts),
        public_title = escape_html(texts.public_section_title),
        public_summary = render_public_summary(&bundle.public_snapshot, texts),
    )
}

fn render_digest_summary(bundle: &IntelligenceWidgetSnapshot, texts: &HostCopy) -> String {
    let items = [
        (texts.metric_visits, bundle.digest_summary.total_visits.value),
        (texts.metric_searches, bundle.digest_summary.total_searches.value),
        (texts.metric_new_sites, bundle.digest_summary.new_domains.value),
        (texts.metric_deep_read, bundle.digest_summary.deep_read_pages.value),
        (texts.metric_refind, bundle.digest_summary.refind_pages.value),
    ];
    let cards = items
        .into_iter()
        .map(|(label, value)| {
            format!(
                r#"<article><span class="label">{}</span><span class="value">{}</span></article>"#,
                escape_html(label),
                value
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(r#"<div class="digest-grid">{cards}</div>"#)
}

fn render_embed_cards(
    cards: &[crate::models::IntelligenceEmbedCardPayload],
    texts: &HostCopy,
) -> String {
    if cards.is_empty() {
        return format!(r#"<p>{}</p>"#, escape_html(texts.no_cards));
    }
    let rendered_cards = cards
        .iter()
        .map(|card| {
            let eyebrow = card
                .eyebrow
                .as_deref()
                .map(|value| format!(r#"<p class="eyebrow">{}</p>"#, escape_html(value)))
                .unwrap_or_default();
            let badge = if card.internal_only {
                format!(r#"<span class="badge">{}</span>"#, escape_html(texts.trusted_only_title))
            } else {
                String::new()
            };
            let metric = match (card.metric_label.as_deref(), card.metric_value.as_deref()) {
                (Some(label), Some(value)) => format!(
                    r#"<p class="metric">{}: {}</p>"#,
                    escape_html(label),
                    escape_html(value)
                ),
                _ => String::new(),
            };
            let href = card
                .href
                .as_deref()
                .map(|href| format!(r#"<p class="metric">{}</p>"#, escape_html(href)))
                .unwrap_or_default();
            format!(
                r#"<article class="card">{eyebrow}<div><h3>{title}</h3>{badge}</div><p>{body}</p>{metric}{href}</article>"#,
                title = escape_html(&card.title),
                badge = badge,
                body = escape_html(&card.body),
                metric = metric,
                href = href,
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(r#"<div class="card-grid">{rendered_cards}</div>"#)
}

fn render_widget_summary(snapshot: &IntelligenceWidgetSnapshot, texts: &HostCopy) -> String {
    let highlights = render_embed_cards(&snapshot.highlights, texts);
    let notes = if snapshot.notes.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="subgrid"><h3>{}</h3><ul class="simple-list">{}</ul></section>"#,
            escape_html(texts.notes_title),
            render_notes(&snapshot.notes)
        )
    };
    format!(
        r#"<div class="subgrid">{digest}{highlights}{notes}</div>"#,
        digest = render_digest_summary(snapshot, texts),
        highlights = highlights,
        notes = notes,
    )
}

fn render_public_summary(snapshot: &IntelligencePublicSnapshot, texts: &HostCopy) -> String {
    let top_domains = if snapshot.top_domains.is_empty() {
        format!(r#"<p>{}</p>"#, escape_html(texts.no_cards))
    } else {
        format!(
            r#"<ul class="simple-list">{}</ul>"#,
            snapshot
                .top_domains
                .iter()
                .map(|domain| format!(r#"<li>{}</li>"#, escape_html(domain)))
                .collect::<Vec<_>>()
                .join("")
        )
    };
    let search_engines = if snapshot.search_engines.is_empty() {
        format!(r#"<p>{}</p>"#, escape_html(texts.no_search_engines))
    } else {
        format!(
            r#"<div class="table-like">{}</div>"#,
            snapshot
                .search_engines
                .iter()
                .map(|engine| {
                    format!(
                        r#"<div class="row"><span>{}</span><span>{}</span></div>"#,
                        escape_html(
                            engine.display_name.as_deref().unwrap_or(&engine.search_engine)
                        ),
                        engine.search_count
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };
    let discovery_trend = if snapshot.discovery_trend.points.is_empty() {
        format!(r#"<p>{}</p>"#, escape_html(texts.no_discovery_trend))
    } else {
        format!(
            r#"<div class="table-like">{}</div>"#,
            snapshot
                .discovery_trend
                .points
                .iter()
                .map(|point| {
                    format!(
                        r#"<div class="row"><span>{}</span><span>{:.2}</span></div>"#,
                        escape_html(&point.date_key),
                        point.discovery_rate
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        )
    };
    let notes = if snapshot.notes.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="subgrid"><h3>{}</h3><ul class="simple-list">{}</ul></section>"#,
            escape_html(texts.notes_title),
            render_notes(&snapshot.notes)
        )
    };
    format!(
        r#"<div class="stats-grid">
  <article class="subgrid"><h3>{top_domains_title}</h3>{top_domains}</article>
  <article class="subgrid"><h3>{search_engines_title}</h3>{search_engines}</article>
  <article class="subgrid"><h3>{discovery_trend_title}</h3>{discovery_trend}</article>
</div>{notes}"#,
        top_domains_title = escape_html(texts.top_domains_title),
        top_domains = top_domains,
        search_engines_title = escape_html(texts.search_engines_title),
        search_engines = search_engines,
        discovery_trend_title = escape_html(texts.discovery_trend_title),
        discovery_trend = discovery_trend,
        notes = notes,
    )
}

fn render_notes(notes: &[String]) -> String {
    notes
        .iter()
        .map(|note| format!(r#"<li>{}</li>"#, escape_html(note)))
        .collect::<Vec<_>>()
        .join("")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        archive::{create_schema, open_archive_connection},
        config::project_paths_with_root,
        intelligence::run_core_intelligence,
        models::{AppConfig, ArchiveMode, CoreIntelligenceRebuildRequest},
    };
    use rusqlite::Connection;

    #[test]
    fn preview_intelligence_local_host_builds_fixed_generated_files() {
        let (_root, paths, config) = prepared_archive();

        let preview = preview_intelligence_local_host(
            &paths,
            &config,
            None,
            &IntelligenceLocalHostRequest {
                date_range: crate::models::DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
                locale: "en".to_string(),
            },
        )
        .expect("preview");

        assert!(
            preview.artifact_root.ends_with("integrations/core-intelligence/browser-snippet-v1")
        );
        assert!(
            preview
                .entry_file_path
                .ends_with("integrations/core-intelligence/browser-snippet-v1/index.html")
        );
        assert_eq!(preview.generated_files.len(), 2);
        assert_eq!(
            preview.generated_files[0].relative_path,
            "integrations/core-intelligence/browser-snippet-v1/index.html"
        );
        assert_eq!(
            preview.generated_files[1].relative_path,
            "integrations/core-intelligence/browser-snippet-v1/bundle.json"
        );
        assert!(preview.generated_files[0].contents.contains("PathKeep Core Intelligence Snippet"));
        assert_eq!(preview.bundle.host_id, BROWSER_SNIPPET_HOST_ID);
        assert_eq!(preview.bundle.bundle_version, LOCAL_HOST_BUNDLE_VERSION);
        assert!(preview.bundle.trusted_only_card_count >= 1);
        assert!(preview.bundle.boundary_notes.iter().any(|note| note.contains("Trusted-only")));
    }

    #[test]
    fn build_intelligence_local_host_writes_artifacts_and_reports_installed_host() {
        let (_root, paths, config) = prepared_archive();

        let result = build_intelligence_local_host(
            &paths,
            &config,
            None,
            &IntelligenceLocalHostRequest {
                date_range: crate::models::DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: Some("chrome:Default".to_string()),
                locale: "en".to_string(),
            },
        )
        .expect("build");

        let installed_host = result.installed_host.expect("installed host");
        assert!(Path::new(&installed_host.entry_file_path).exists());
        assert!(Path::new(&installed_host.artifact_root).join("bundle.json").exists());
        let saved_bundle =
            fs::read_to_string(Path::new(&installed_host.artifact_root).join("bundle.json"))
                .expect("read bundle");
        let parsed_bundle: IntelligenceLocalHostBundle =
            serde_json::from_str(&saved_bundle).expect("parse bundle");
        assert_eq!(parsed_bundle.date_range, result.bundle.date_range);
        assert_eq!(parsed_bundle.profile_id, result.bundle.profile_id);
        assert_eq!(parsed_bundle.trusted_only_card_ids, result.bundle.trusted_only_card_ids);
        assert_eq!(
            parsed_bundle.public_snapshot.top_domains,
            result.bundle.public_snapshot.top_domains
        );
        assert!(
            fs::read_to_string(&installed_host.entry_file_path)
                .expect("read html")
                .contains("Trusted-only cards are still present")
        );
    }

    #[test]
    fn installed_host_loader_reports_missing_corrupt_and_valid_artifacts() {
        let (_root, paths, config) = prepared_archive();
        let artifact_root = local_host_root(&paths);
        let bundle_path = artifact_root.join("bundle.json");
        let entry_path = artifact_root.join("index.html");

        let (empty, warnings) = load_installed_host(&paths, "en").expect("empty load");
        assert!(empty.is_none());
        assert!(warnings.is_empty());

        fs::create_dir_all(&artifact_root).expect("artifact root");
        fs::create_dir_all(&bundle_path).expect("bundle path as dir");
        let (missing, warnings) = load_installed_host(&paths, "zh-TW").expect("read failure");
        assert!(missing.is_none());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("無法讀取"));
        fs::remove_dir(&bundle_path).expect("remove bundle dir");

        fs::write(&bundle_path, "{not-json").expect("corrupt bundle");
        let (missing, warnings) = load_installed_host(&paths, "zh-CN").expect("parse failure");
        assert!(missing.is_none());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("无法解析"));

        let built = build_intelligence_local_host(
            &paths,
            &config,
            None,
            &IntelligenceLocalHostRequest {
                date_range: crate::models::DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: None,
                locale: "en".to_string(),
            },
        )
        .expect("build");
        fs::remove_file(&entry_path).expect("remove entry");
        let (missing, warnings) = load_installed_host(&paths, "en").expect("missing entry");
        assert!(missing.is_none());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("index.html"));

        fs::write(&entry_path, "ok").expect("restore entry");
        let (installed, warnings) = load_installed_host(&paths, "en").expect("valid load");
        assert!(warnings.is_empty());
        assert_eq!(
            installed.expect("installed").bundle.bundle_version,
            built.bundle.bundle_version
        );
    }

    #[test]
    fn local_host_rendering_covers_locales_empty_states_and_html_escaping() {
        assert!(host_copy("zh-TW").summary_title.contains("受信任"));
        assert!(host_copy("zh-CN").summary_title.contains("受信任"));
        assert!(host_copy("en-US").summary_title.contains("Trusted"));
        assert!(build_boundary_notes("zh-TW")[0].contains("deterministic"));
        assert!(build_boundary_notes("zh-CN")[1].contains("trusted-only"));

        let (_root, paths, config) = prepared_archive();
        let mut bundle = preview_intelligence_local_host(
            &paths,
            &config,
            None,
            &IntelligenceLocalHostRequest {
                date_range: crate::models::DateRange {
                    start: "2024-04-01".to_string(),
                    end: "2024-04-30".to_string(),
                },
                profile_id: None,
                locale: "en".to_string(),
            },
        )
        .expect("preview")
        .bundle;
        bundle.locale = "zh-CN".to_string();
        bundle.profile_id = Some("chrome:<Default>".to_string());
        bundle.embed_cards.clear();
        bundle.widget_snapshot.highlights.clear();
        bundle.widget_snapshot.notes.clear();
        bundle.public_snapshot.top_domains.clear();
        bundle.public_snapshot.search_engines.clear();
        bundle.public_snapshot.discovery_trend.points.clear();
        bundle.public_snapshot.notes.clear();
        let html = render_browser_snippet_html(&bundle, &host_copy("zh-CN"));
        assert!(html.contains("Profile: chrome:&lt;Default&gt;"));
        assert!(html.contains("暂时没有可用的卡片"));
        assert!(html.contains("没有可用的搜索引擎活动"));
        assert!(html.contains("没有可用的发现趋势点"));

        let plain_card = crate::models::IntelligenceEmbedCardPayload {
            card_id: "plain".to_string(),
            card_type: "summary".to_string(),
            title: "A <plain> card".to_string(),
            body: "Body & detail".to_string(),
            internal_only: false,
            ..Default::default()
        };
        let cards = render_embed_cards(&[plain_card], &host_copy("en"));
        assert!(cards.contains("A &lt;plain&gt; card"));
        assert!(!cards.contains("badge"));
        assert!(!cards.contains("metric"));
    }

    fn prepared_archive() -> (tempfile::TempDir, ProjectPaths, AppConfig) {
        let root = tempfile::tempdir().expect("tempdir");
        let paths = project_paths_with_root(root.path());
        let config = AppConfig {
            initialized: true,
            archive_mode: ArchiveMode::Plaintext,
            ..AppConfig::default()
        };
        let archive = open_archive_connection(&paths, &config, None).expect("archive");
        create_schema(&archive).expect("schema");
        seed_core_intelligence_fixture(&archive);
        drop(archive);
        run_core_intelligence(&paths, &config, None, &CoreIntelligenceRebuildRequest::default())
            .expect("run core intelligence");
        (root, paths, config)
    }

    fn seed_core_intelligence_fixture(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO runs (
                    id, run_type, trigger, started_at, timezone, status, profile_scope_json, warnings_json, stats_json, due_only
                 ) VALUES (
                    1, 'backup', 'manual', '2026-04-14T00:00:00Z', 'UTC', 'success', '[]', '[]', '{}', 0
                 )",
                [],
            )
            .expect("run");
        connection
            .execute(
                "INSERT INTO source_profiles (
                    id, browser_kind, browser_version, profile_name, profile_path, discovered_at, enabled, profile_key, updated_at
                 ) VALUES (
                    1, 'chrome', '1', 'Default', '/tmp/profile', '2026-04-14T00:00:00Z', 1, 'chrome:Default', '2026-04-14T00:00:00Z'
                 )",
                [],
            )
            .expect("profile");
        connection
            .execute(
                "INSERT INTO urls (
                    id, url, title, visit_count, typed_count, first_visit_ms, first_visit_iso, last_visit_ms, last_visit_iso,
                    source_profile_id, created_by_run_id, source_url_id, hidden, payload_hash, recorded_at
                 ) VALUES
                 (1, 'https://www.google.com/search?q=sqlite+wal', 'sqlite wal - Google Search', 1, 0, 1, '1970-01-01T00:00:00Z', 1, '1970-01-01T00:00:00Z', 1, 1, 11, 0, 'hash-1', '2026-04-14T00:00:00Z'),
                 (2, 'https://github.com/example/repo/issues/42', 'Issue 42', 2, 1, 2, '1970-01-01T00:00:02Z', 86400002, '1970-01-02T00:00:00Z', 1, 1, 12, 0, 'hash-2', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("urls");
        connection
            .execute(
                "INSERT INTO visits (
                    id, url_id, source_visit_id, visit_time_ms, visit_time_iso, transition_type, visit_duration_ms,
                    source_profile_id, created_by_run_id, from_visit, is_known_to_sync, event_fingerprint, payload_hash, recorded_at
                 ) VALUES
                 (1, 1, '1', 1711929600000, '2024-04-01T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-1', 'visit-hash-1', '2026-04-14T00:00:00Z'),
                 (2, 2, '2', 1711929660000, '2024-04-01T00:01:00Z', 1, 0, 1, 1, 1, 0, 'fingerprint-2', 'visit-hash-2', '2026-04-14T00:00:00Z'),
                 (3, 2, '3', 1712016000000, '2024-04-02T00:00:00Z', 1, 0, 1, 1, NULL, 0, 'fingerprint-3', 'visit-hash-3', '2026-04-14T00:00:00Z')",
                [],
            )
            .expect("visits");
        connection
            .execute(
                "INSERT INTO search_terms (
                    id, url_id, term, normalized_term, source_profile_id, created_by_run_id, profile_id
                 ) VALUES (
                    1, 1, 'sqlite wal', 'sqlite wal', 1, 1, 'chrome:Default'
                 )",
                [],
            )
            .expect("search term");
    }
}
