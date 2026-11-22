/**
 * Settings → 關於. Mirrors `src/features/settings/about-section.tsx`: which
 * build this is, updates, logs, crash reports, the copyable diagnostics and
 * the archive health check. No design change here.
 */
import { useState } from 'react'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { CrashReportsRow, DiagnosticsReportRow, LogsRow, type CrashState } from './about/diagnostics-rows'
import { HealthRow, type HealthOutcome } from './about/health-row'
import { UpdateRow, updateStateLabels, type UpdateState } from './about/update-row'
import { PlatformSelect, ProtoBar, ProtoSelect, ProtoSwitch } from './pathkeep-kit/proto-bar'

export function AboutSection() {
  const [dirty, setDirty] = useState(false)
  const [update, setUpdate] = useState<UpdateState>('idle')
  const [downloaded, setDownloaded] = useState(0)
  const [crash, setCrash] = useState<CrashState>('none')
  const [health, setHealth] = useState<HealthOutcome>('none')
  const [healthNext, setHealthNext] = useState<'pass' | 'fail'>('fail')

  return (
    <SettingsSection title="關於">
      <ProtoBar>
        <PlatformSelect />
        <ProtoSwitch label="有未提交的修改" checked={dirty} onChange={setDirty} />
        <ProtoSelect<UpdateState>
          label="更新"
          value={update}
          onChange={(next) => {
            setDownloaded(next === 'downloading' ? 15_200_000 : next === 'installing' || next === 'installed' ? 40_480_000 : 0)
            setUpdate(next)
          }}
          options={(Object.keys(updateStateLabels) as UpdateState[]).map((value) => ({ value, label: updateStateLabels[value] }))}
        />
        <ProtoSelect<CrashState>
          label="當機"
          value={crash}
          onChange={setCrash}
          options={[
            { value: 'none', label: '沒有' },
            { value: 'latest', label: '有一次' },
            { value: 'fatal', label: '有一次，PathKeep 關閉了' },
          ]}
        />
        <ProtoSelect<HealthOutcome>
          label="檢查存檔"
          value={health}
          onChange={(next) => {
            setHealth(next)
            if (next !== 'none') setHealthNext(next)
          }}
          options={[
            { value: 'none', label: '還沒檢查' },
            { value: 'pass', label: '全部通過' },
            { value: 'fail', label: '有問題' },
          ]}
        />
      </ProtoBar>
      <SettingRow
        title="版本"
        description={
          <span className="font-mono text-xs">
            PathKeep 0.6.0-dev · a88d4fb
            {dirty && ' （已修改）'}
          </span>
        }
      />
      <UpdateRow state={update} onState={setUpdate} downloaded={downloaded} onDownloaded={setDownloaded} />
      <LogsRow />
      <CrashReportsRow state={crash} />
      <DiagnosticsReportRow dirty={dirty} crashState={crash} />
      <HealthRow
        outcome={health}
        failNext={healthNext === 'fail'}
        onOutcome={(next) => {
          setHealth(next)
          if (next !== 'none') setHealthNext(next)
        }}
      />
    </SettingsSection>
  )
}
