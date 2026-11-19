/**
 * Responsibilities: the conversation bank: short coding conversations in
 * several languages, the follow-up turns the append mutator adds, and the
 * model names each tool records.
 * Not responsible for: how a tool stores a turn, or when it happened.
 */

export const MODELS = {
  'claude-code': ['claude-opus-4-5-20251101', 'claude-sonnet-4-5-20250929'],
  codex: ['gpt-5.1-codex', 'gpt-5.1-codex-max'],
  deepseek: ['deepseek-chat', 'deepseek-reasoner'],
}

export const CLAUDE_SUBAGENT_MODEL = 'claude-haiku-4-5-20251001'

/** pi's providers, with the price per million tokens it records cost from. */
export const PI_MODELS = [
  {
    provider: 'anthropic',
    model: 'claude-sonnet-4-5',
    api: 'anthropic-messages',
    price: { input: 3, output: 15, cacheRead: 0.3, cacheWrite: 3.75 },
  },
  {
    provider: 'openai',
    model: 'gpt-5.1',
    api: 'openai-responses',
    price: { input: 1.25, output: 10, cacheRead: 0.125, cacheWrite: 0 },
  },
]

/**
 * Short coding conversations. A turn is the person's message, then the
 * agent's steps: a string is a reply, `run`/`read` a tool call and its output.
 * `{w}` becomes the conversation's unique word.
 */
export const TEMPLATES = [
  {
    stacks: ['rust'],
    turns: [
      [
        'The `{w}` test times out on CI but passes locally. Can you find out why?',
        'Let me run it on its own first.',
        {
          run: 'cargo test {w} -- --nocapture',
          out: "running 1 test\ntest import::{w} ... FAILED\n\nthread 'import::{w}' panicked at tests/import.rs:88:5:\ntimed out after 60s waiting for the import lock\n\nfailures:\n    import::{w}\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored",
        },
        'The test before it keeps the import lock guard alive until the end of its function, so this one waits for the whole timeout. I moved the guard into its own scope; the test passes in 0.4 s now.',
      ],
      [
        'Could the same thing explain the flaky run last week?',
        'Very likely. That run failed in the same test with the same message, and the order only matters when both tests land on the same thread.',
      ],
    ],
  },
  {
    stacks: ['rust'],
    turns: [
      [
        'Add a `--{w}` flag that prints the tide table as JSON instead of the text table.',
        {
          read: 'src/main.rs',
          out: 'use clap::Parser;\n\n#[derive(Parser)]\nstruct Args {\n    /// Station id, e.g. 9414290\n    station: String,\n    #[arg(long, default_value_t = 7)]\n    days: u32,\n}\n\nfn main() -> anyhow::Result<()> {\n    let args = Args::parse();\n    let table = tides::predict(&args.station, args.days)?;\n    print!("{table}");\n    Ok(())\n}',
        },
        'Added `--{w}` to `Args`. When it is set the table goes through `serde_json::to_writer` on a locked stdout, so a long table streams instead of being built as one string. The text output is unchanged.',
      ],
    ],
  },
  {
    stacks: ['rust'],
    turns: [
      [
        'clippy warns about `needless_collect` in `{w}.rs`. Fix it without changing behaviour.',
        {
          run: 'cargo clippy --all-targets -- -D warnings',
          out: 'warning: avoid using `collect()` when not needed\n  --> src/{w}.rs:41:10\n   |\n41 |         .collect::<Vec<_>>();\n   |          ^^^^^^^\n   = note: `#[warn(clippy::needless_collect)]` on by default',
        },
        'The vector was only used for `.len()` and one loop. I replaced it with `count()` on a cloned iterator, which keeps the side effects in the same order. clippy is clean now.',
      ],
    ],
  },
  {
    stacks: ['rust'],
    turns: [
      [
        'The release binary is 41 MB. Check the profile, I think the `{w}` feature pulls in too much.',
        {
          run: 'cargo bloat --release --crates -n 6',
          out: 'File  .text     Size Crate\n18.2%  31.0%  7.5MiB wasmtime\n 9.1%  15.5%  3.7MiB {w}\n 6.3%  10.7%  2.6MiB std\n 4.0%   6.8%  1.6MiB tokio\n 2.2%   3.7%  0.9MiB regex',
        },
        'Most of it is wasmtime, which only the `{w}` feature needs. With `strip = true`, `lto = "thin"` and that feature off by default, the binary is 14 MB.',
      ],
    ],
  },
  {
    stacks: ['web'],
    turns: [
      [
        'The sidebar flickers when I switch routes. It started after the `{w}` refactor.',
        'The refactor moved the sidebar inside the route outlet, so every navigation mounts it again.',
        {
          read: 'src/app/layout.tsx',
          out: 'export function Layout() {\n  return <Outlet context={{ sidebar: <Sidebar /> }} />\n}',
        },
        'I moved `<Sidebar />` back into the layout, next to the outlet. It keeps its state across routes now and the flicker is gone.',
      ],
    ],
  },
  {
    stacks: ['web'],
    turns: [
      [
        'Write a Playwright test that turns on the `{w}` toggle in settings and checks it survives a reload.',
        'Added `tests/e2e/settings-{w}.spec.ts`: it flips the switch, waits for the save, reloads and checks the switch is still on.',
        {
          run: 'npx playwright test settings-{w}',
          out: 'Running 1 test using 1 worker\n\n  ✓  1 settings-{w}.spec.ts:4:1 › toggle survives a reload (2.1s)\n\n  1 passed (3.4s)',
        },
        'It passes. The test finds the switch by its role and name, not by a CSS class.',
      ],
    ],
  },
  {
    stacks: ['web'],
    turns: [
      [
        'The post list re-renders every card when one `{w}` tag changes. Make it cheaper.',
        'Each card got a new `tags` array on every render because the filter ran inside the map. I moved the filter into a memo keyed by the tag list and wrapped the card in `memo`, so changing one tag re-renders one card.',
      ],
      [
        'Did you check it in the profiler?',
        'Yes. Changing a tag went from 212 card renders to 1, and the commit from 38 ms to 3 ms.',
      ],
    ],
  },
  {
    stacks: ['web'],
    turns: [
      [
        'Code blocks are unreadable in dark mode. The `{w}` theme should use our color tokens.',
        {
          read: 'src/styles/code.css',
          out: '.code {\n  background: #f6f8fa;\n  color: #24292f;\n}',
        },
        'The colors were fixed hex values. They now use `--surface-muted` and `--text`, which already have dark values; contrast is 12.6:1 in dark mode.',
      ],
    ],
  },
  {
    stacks: ['python'],
    turns: [
      [
        '記帳本的 `{w}` 匯出把金額四捨五入錯了，0.5 元都被捨掉，幫我看一下',
        {
          run: 'python -m pytest tests/test_export.py -k {w}',
          out: 'FAILED tests/test_export.py::test_{w}_rounding - assert 12.0 == 13.0\n1 failed, 14 passed in 0.31s',
        },
        '原因是 `round()` 用的是銀行家捨入，12.5 會變成 12。我改成 `Decimal.quantize(ROUND_HALF_UP)`，測試都過了。',
      ],
    ],
  },
  {
    stacks: ['python'],
    turns: [
      [
        '帮我给 `{w}` 函数写单元测试，覆盖负数和空列表',
        '加好了 `tests/test_{w}.py`，一共四个用例：正常数据、负数、空列表、只有一项。空列表原来会抛 `ZeroDivisionError`，我顺手让它返回 0。',
      ],
    ],
  },
  {
    stacks: ['python'],
    turns: [
      [
        'pytest の `{w}` テストが Windows だけで失敗します。パス区切りが原因ですか？',
        'はい。期待値が `"data/2026/11.csv"` と書かれていて、Windows では区切りが `\\` になります。`Path` で組み立てて比べるように直しました。',
      ],
    ],
  },
  {
    stacks: ['ops'],
    turns: [
      [
        'The nightly `{w}` backup did not run last night. Check the systemd timer.',
        {
          run: 'systemctl list-timers --all | grep {w}',
          out: 'n/a  n/a  Tue 2026-11-17 02:00:00 UTC  20h ago  {w}-backup.timer  {w}-backup.service',
        },
        'The timer fired, but the service failed: the disk check exits 1 above 85 % and the volume was at 87 %. I removed the old snapshots; it is at 61 % and a manual run finished.',
      ],
    ],
  },
  {
    stacks: ['ops'],
    turns: [
      [
        'Terraform wants to recreate the `{w}` bucket. I only changed tags. Why?',
        {
          run: 'terraform plan -target=aws_s3_bucket.{w}',
          out: '  # aws_s3_bucket.{w} must be replaced\n-/+ resource "aws_s3_bucket" "{w}" {\n      ~ bucket = "{w}-logs" -> "{w}-logs-v2" # forces replacement\n      ~ tags   = { "team" = "infra" -> "platform" }\n    }\n\nPlan: 1 to add, 0 to change, 1 to destroy.',
        },
        'It is not the tags: the bucket name changed too, and a new name forces a replacement. I put the old name back; the plan now only updates tags.',
      ],
    ],
  },
  {
    stacks: ['ops'],
    turns: [
      [
        '把 `{w}` 部署腳本改成失敗時自動回滾到上一個版本',
        '改好了：部署前先記下目前的 release 目錄，健康檢查連續三次失敗就把 symlink 指回去並重啟服務。另外加了 `--no-rollback` 給手動除錯用。',
      ],
    ],
  },
  {
    stacks: ['rust', 'web', 'python', 'ops'],
    turns: [
      [
        'Explain what the `{w}` module does. I have not touched it in months.',
        'It keeps a small cache of parsed configs keyed by file path and modification time. Readers share one copy; a write drops the entry and the next read parses again. The settings screen is the only caller left.',
      ],
    ],
  },
  {
    stacks: ['rust', 'web', 'python', 'ops'],
    turns: [
      [
        'Write a commit message for the staged changes. The ticket is `{w}`.',
        {
          run: 'git diff --cached --stat',
          out: ' src/import/queue.rs | 42 ++++++++++++++++++++++-------\n src/import/mod.rs   |  6 ++--\n 2 files changed, 37 insertions(+), 11 deletions(-)',
        },
        'fix(import): retry a queued file once before giving up ({w})\n\nA file that is still being written fails the first read. The queue now waits two seconds and tries once more before marking it failed.',
      ],
    ],
  },
]

/** Follow-up turns the append mutator adds; `{w}` is a new unique word. */
export const FOLLOW_UPS = [
  [
    'One more thing: name the new setting `{w}` and mention it in the changelog.',
    'Done. The setting is `{w}` and the changelog has a line under "Changed".',
  ],
  [
    'Can you also add a short comment above `{w}` saying why it exists?',
    'Added a two-line comment above `{w}` that says what breaks without it.',
  ],
]

export function turnsFrom(template, word) {
  const fill = (text) => text.replaceAll('{w}', word)
  return template.map(([user, ...steps]) => ({
    user: fill(user),
    steps: steps.map((step) =>
      typeof step === 'string'
        ? { text: fill(step) }
        : step.run
          ? { tool: 'run', arg: fill(step.run), out: fill(step.out) }
          : { tool: 'read', arg: step.read, out: fill(step.out) },
    ),
  }))
}
