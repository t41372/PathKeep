//! Responsibilities: keep CLI help and failure messages available in all shipped locales.
//! Not responsible for: translating stable machine-readable protocol values.
use std::ffi::OsString;

#[derive(Clone, Copy)]
pub(super) enum Locale {
    English,
    SimplifiedChinese,
    TraditionalChinese,
}

impl Locale {
    /// Even malformed commands retain their requested language without echoing argv values.
    pub fn from_arguments(arguments: &[OsString]) -> Self {
        arguments
            .windows(2)
            .find_map(|pair| {
                if pair[0] != "--locale" {
                    return None;
                }
                Some(match pair[1].to_str() {
                    Some("zh-CN") => Self::SimplifiedChinese,
                    Some("zh-TW") => Self::TraditionalChinese,
                    _ => Self::English,
                })
            })
            .unwrap_or(Self::English)
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::English => "Back up, inspect and export local agent conversations.",
            Self::SimplifiedChinese => "备份、查看和导出本机 Agent 对话记录。",
            Self::TraditionalChinese => "備份、檢視與匯出本機 Agent 對話記錄。",
        }
    }

    pub fn guidance(self) -> &'static str {
        match self {
            Self::English => {
                "Use plan to preview sources. Backup, reparse and export require --execute. Read the archive password from standard input with --password-stdin; do not put it in arguments. Exports never replace existing files. Use sync-search --rebuild to recreate a damaged search index. Reparse reads retained evidence without accessing the original source. Use sources to find retained artifacts, including those that could not be parsed."
            }
            Self::SimplifiedChinese => {
                "使用 plan 预览来源。备份、重新解析和导出需要 --execute。使用 --password-stdin 从标准输入读取存档密码，不要把密码放在命令参数中。导出不会覆盖现有文件。使用 sync-search --rebuild 重建损坏的搜索索引。重新解析只读取已备份的数据，不会访问原始来源。使用 sources 查找保留的原始文件，包括无法解析的文件。"
            }
            Self::TraditionalChinese => {
                "使用 plan 預覽來源。備份、重新解析與匯出需要 --execute。使用 --password-stdin 從標準輸入讀取封存密碼，不要把密碼放在命令參數中。匯出不會覆寫現有檔案。使用 sync-search --rebuild 重建損壞的搜尋索引。重新解析只讀取已備份的資料，不會存取原始來源。使用 sources 尋找保留的原始檔案，包括無法解析的檔案。"
            }
        }
    }

    pub fn usage(self) -> String {
        let labels = match self {
            Self::English => [
                "Commands",
                "Common",
                "Sources",
                "Queries",
                "Events",
                "Search",
                "Revisions",
                "Export",
                "Raw export",
                "Reparse",
                "Retained sources",
            ],
            Self::SimplifiedChinese => [
                "命令",
                "通用参数",
                "来源",
                "查询",
                "事件",
                "搜索",
                "版本",
                "导出",
                "原始数据导出",
                "重新解析",
                "保留的来源",
            ],
            Self::TraditionalChinese => [
                "命令",
                "通用參數",
                "來源",
                "查詢",
                "事件",
                "搜尋",
                "版本",
                "匯出",
                "原始資料匯出",
                "重新解析",
                "保留的來源",
            ],
        };
        format!(
            "pathkeep-agent-history <command> [options]\n{}: init, plan, backup, reparse, list, sources, events, revisions, search, export, raw-export, verify, sync-search\n{}: --archive PATH --password-stdin --locale en|zh-CN|zh-TW\n{}: --source HARNESS=ROOT\n{}: --harness NAME --project PATH --limit 1..500 --after CURSOR --sort updated|created\n{}: --session ID\n{}: --query TEXT --limit 1..100\n{}: --source-id ID\n{}: --session ID --format jsonl|markdown --output PATH --execute\n{}: --revision ID --output PATH --execute\n{}: reparse --source-id ID --execute\n{}: sources --harness NAME --kind transcript|attachment|metadata|sqlite --limit 1..500 --after CURSOR",
            labels[0],
            labels[1],
            labels[2],
            labels[3],
            labels[4],
            labels[5],
            labels[6],
            labels[7],
            labels[8],
            labels[9],
            labels[10],
        )
    }

    pub fn error(self, code: &str) -> &'static str {
        let messages = match code {
            "invalid_input" => [
                "Check the command and its required arguments.",
                "请检查命令及其必需参数。",
                "請檢查命令及其必要參數。",
            ],
            "execute_required" => [
                "Preview with plan, then add --execute to perform this operation.",
                "请先使用 plan 预览，再添加 --execute 执行操作。",
                "請先使用 plan 預覽，再加入 --execute 執行操作。",
            ],
            "wrong_password" => [
                "The password is incorrect or the key envelope was changed.",
                "密码不正确，或密钥信封已被更改。",
                "密碼不正確，或金鑰信封已被更改。",
            ],
            "invalid_envelope" => [
                "The archive key envelope cannot be read.",
                "无法读取存档密钥信封。",
                "無法讀取封存金鑰信封。",
            ],
            "io_error" => [
                "The file operation failed. Check access permissions and available space.",
                "文件操作失败，请检查访问权限和可用空间。",
                "檔案操作失敗，請檢查存取權限與可用空間。",
            ],
            "storage_error" => [
                "The archive storage operation failed.",
                "存档存储操作失败。",
                "封存儲存操作失敗。",
            ],
            "invalid_json" => [
                "The data could not be decoded as JSON.",
                "无法将数据解析为 JSON。",
                "無法將資料解析為 JSON。",
            ],
            "integrity_error" => [
                "The archive failed an integrity check.",
                "存档未通过完整性检查。",
                "封存未通過完整性檢查。",
            ],
            "not_found" => [
                "The requested record or file was not found.",
                "找不到请求的记录或文件。",
                "找不到要求的記錄或檔案。",
            ],
            "already_exists" => [
                "The destination already exists. Choose a new destination.",
                "目标已存在，请选择新位置。",
                "目的地已存在，請選擇新位置。",
            ],
            "invalid_archive" => [
                "This directory is not a supported agent history archive.",
                "此目录不是受支持的 Agent 历史存档。",
                "此目錄不是支援的 Agent 歷史封存。",
            ],
            "unsupported" => [
                "This operation is not supported for the selected data.",
                "所选数据不支持此操作。",
                "所選資料不支援此操作。",
            ],
            "limit_exceeded" => [
                "The operation exceeded its configured resource limit.",
                "操作超出已配置的资源限制。",
                "操作超出已設定的資源限制。",
            ],
            "source_changed" => [
                "The source changed during capture. Retry after its writer is idle.",
                "来源在备份过程中发生变化，请在写入停止后重试。",
                "來源在備份過程中發生變化，請在寫入停止後重試。",
            ],
            "index_behind" => [
                "The search index is not current. Run sync-search before searching.",
                "搜索索引尚未更新，请先运行 sync-search。",
                "搜尋索引尚未更新，請先執行 sync-search。",
            ],
            "stale_cursor" => [
                "The data changed after this page was read. Start from the first page.",
                "读取此页后数据已变化，请从第一页重新开始。",
                "讀取此頁後資料已變化，請從第一頁重新開始。",
            ],
            "cancelled" => ["The operation was cancelled.", "操作已取消。", "操作已取消。"],
            _ => ["The operation could not be completed.", "无法完成操作。", "無法完成操作。"],
        };
        messages[match self {
            Self::English => 0,
            Self::SimplifiedChinese => 1,
            Self::TraditionalChinese => 2,
        }]
    }
}
