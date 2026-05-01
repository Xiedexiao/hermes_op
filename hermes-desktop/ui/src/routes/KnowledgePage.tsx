import { useEffect, useMemo, useState } from "react";
import {
  knowledgeFetchUrlPreview,
  knowledgeImport,
  knowledgeImportFolder,
  knowledgeList,
  knowledgeSourceList,
  missionList,
  type KnowledgeFeedItem,
  type KnowledgeSource,
} from "../lib/tauri";
import { useMissionStore } from "../store/missionStore";
import "./KnowledgePage.css";

type KnowledgeImportMode = "note" | "url" | "file" | "folder";

export function KnowledgePage() {
  const missions = useMissionStore((state) => state.missions);
  const setMissions = useMissionStore((state) => state.setMissions);
  const [items, setItems] = useState<KnowledgeFeedItem[]>([]);
  const [sources, setSources] = useState<KnowledgeSource[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [fetchingPreview, setFetchingPreview] = useState(false);
  const [importSummary, setImportSummary] = useState<string | null>(null);
  const [fetchedPreviewMeta, setFetchedPreviewMeta] = useState<{
    status: number;
    contentType?: string | null;
    fetchedAt: string;
    truncated: boolean;
  } | null>(null);
  const [importForm, setImportForm] = useState({
    missionId: "",
    mode: "note" as KnowledgeImportMode,
    title: "",
    preview: "",
    source: "",
    folderPath: "",
    titlePrefix: "",
    recursive: false,
    maxFiles: "20",
  });

  useEffect(() => {
    void loadKnowledge();
  }, [query]);

  useEffect(() => {
    setImportForm((current) => ({
      ...current,
      missionId: current.missionId || missions[0]?.id || "",
    }));
  }, [missions]);

  useEffect(() => {
    if (missions.length > 0) {
      return;
    }

    let cancelled = false;

    async function loadMissions() {
      try {
        const data = await missionList({ limit: 50 });
        if (!cancelled) {
          setMissions(data);
        }
      } catch {
        // keep import area usable only when mission data is available
      }
    }

    void loadMissions();
    return () => {
      cancelled = true;
    };
  }, [missions.length, setMissions]);

  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? items[0] ?? null,
    [items, selectedId],
  );
  const isUrlMode = importForm.mode === "url";
  const isFolderMode = importForm.mode === "folder";
  const importModeCopy = {
    note: {
      sourcePlaceholder: "Optional source",
      previewPlaceholder: "Preview or summary",
      helper:
        "Note 会直接作为 Mission context 和 knowledge source 持久化。",
      button: "Attach Note",
    },
    url: {
      sourcePlaceholder: "https://example.com",
      previewPlaceholder:
        "Fetched page preview will appear here; you can still edit before attach",
      helper:
        "URL fetch preview connector 已可用：先抓取网页 title/summary 到表单，再由你确认后 Attach URL 入库。",
      button: "Attach URL",
    },
    file: {
      sourcePlaceholder: "/absolute/path/to/file.md",
      previewPlaceholder:
        "Optional override. Leave empty to let Rust read a local UTF-8 file.",
      helper:
        "File 导入由 Rust 命令层读取本地 UTF-8 文件并切分入库；前端不会直接访问文件系统。",
      button: "Import Local File",
    },
    folder: {
      helper:
        "Folder 导入只读取本地目录中的 UTF-8 文本/markdown/json/csv/txt 文件；这是本地文件夹导入，不是远端 connector。",
      button: "Import Local Folder",
    },
  }[importForm.mode];

  async function loadKnowledge() {
    setLoading(true);
    setError(null);
    try {
      const [feedData, sourceData] = await Promise.all([
        knowledgeList(query.trim() ? { query } : undefined),
        knowledgeSourceList(query.trim() ? { query } : undefined),
      ]);
      setItems(feedData);
      setSources(sourceData);
      setSelectedId((current) =>
        current && feedData.some((item) => item.id === current)
          ? current
          : (feedData[0]?.id ?? null),
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleFetchUrlPreview() {
    setFetchingPreview(true);
    setError(null);
    setImportSummary(null);
    try {
      const preview = await knowledgeFetchUrlPreview({
        url: importForm.source,
      });
      setImportForm((current) => ({
        ...current,
        source: preview.url,
        title: preview.title?.trim() || current.title,
        preview: preview.preview,
      }));
      setFetchedPreviewMeta({
        status: preview.status,
        contentType: preview.content_type,
        fetchedAt: preview.fetched_at,
        truncated: preview.truncated,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setFetchingPreview(false);
    }
  }

  async function handleImport(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setImporting(true);
    setError(null);
    setImportSummary(null);
    try {
      if (isFolderMode) {
        const parsedMaxFiles = Number.parseInt(importForm.maxFiles, 10);
        const result = await knowledgeImportFolder({
          mission_id: importForm.missionId,
          folder_path: importForm.folderPath,
          title_prefix: importForm.titlePrefix || null,
          recursive: importForm.recursive,
          max_files: Number.isFinite(parsedMaxFiles) ? parsedMaxFiles : null,
        });
        setImportForm((current) => ({
          ...current,
          folderPath: "",
          titlePrefix: "",
          recursive: false,
          maxFiles: "20",
        }));
        setImportSummary(result.summary);
      } else {
        const importType: "note" | "url" | "file" =
          importForm.mode === "note"
            ? "note"
            : importForm.mode === "url"
              ? "url"
              : "file";
        await knowledgeImport({
          mission_id: importForm.missionId,
          type: importType,
          title: importForm.title,
          content_preview: importForm.preview || null,
          source_uri: importForm.source || null,
        });
        setImportForm((current) => ({
          ...current,
          title: "",
          preview: "",
          source: "",
        }));
      }
      setFetchedPreviewMeta(null);
      await loadKnowledge();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setImporting(false);
    }
  }

  return (
    <div className="knowledge-page">
      <div className="knowledge-header">
        <div>
          <h2>Knowledge</h2>
          <p>
            跨 Mission 聚合知识 feed，并显示已持久化的 Knowledge Sources
            与索引状态。
          </p>
        </div>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search knowledge and sources"
          aria-label="Search knowledge and sources"
        />
      </div>

      {error ? <div className="knowledge-empty">{error}</div> : null}
      <section className="knowledge-import-card">
        <div className="knowledge-import-copy">
          <h3>Import Area</h3>
          <p>
            把笔记、URL、本地 UTF-8 文件或本地文件夹导入某个 Mission，写入真实的
            context item、knowledge source 和 chunks。
          </p>
          <div className="knowledge-import-note">{importModeCopy.helper}</div>
        </div>
        <form className="knowledge-import-form" onSubmit={handleImport}>
          <select
            value={importForm.missionId}
            onChange={(event) =>
              setImportForm((current) => ({
                ...current,
                missionId: event.target.value,
              }))
            }
          >
            <option value="" disabled>
              Select mission
            </option>
            {missions.map((mission) => (
              <option key={mission.id} value={mission.id}>
                {mission.title}
              </option>
            ))}
          </select>
          <select
            value={importForm.mode}
            onChange={(event) =>
              {
                const nextType = event.target.value as KnowledgeImportMode;
                setImportForm((current) => ({
                  ...current,
                  mode: nextType,
                }));
                setImportSummary(null);
                if (nextType !== "url") {
                  setFetchedPreviewMeta(null);
                }
              }
            }
          >
            <option value="note">Note</option>
            <option value="url">URL</option>
            <option value="file">File</option>
            <option value="folder">Folder</option>
          </select>
          {isFolderMode ? (
            <>
              <input
                type="text"
                value={importForm.folderPath}
                onChange={(event) =>
                  setImportForm((current) => ({
                    ...current,
                    folderPath: event.target.value,
                  }))
                }
                placeholder="/absolute/path/to/folder"
                required
              />
              <input
                type="text"
                value={importForm.titlePrefix}
                onChange={(event) =>
                  setImportForm((current) => ({
                    ...current,
                    titlePrefix: event.target.value,
                  }))
                }
                placeholder="Optional title prefix"
              />
              <div className="knowledge-inline-fields">
                <label className="knowledge-checkbox">
                  <input
                    type="checkbox"
                    checked={importForm.recursive}
                    onChange={(event) =>
                      setImportForm((current) => ({
                        ...current,
                        recursive: event.target.checked,
                      }))
                    }
                  />
                  <span>Recursive import</span>
                </label>
                <input
                  type="number"
                  min={1}
                  max={100}
                  value={importForm.maxFiles}
                  onChange={(event) =>
                    setImportForm((current) => ({
                      ...current,
                      maxFiles: event.target.value,
                    }))
                  }
                  placeholder="Max files (default 20)"
                />
              </div>
              <button
                type="submit"
                disabled={importing || !importForm.missionId}
              >
                {importing ? "Importing..." : importModeCopy.button}
              </button>
            </>
          ) : (
            <>
              <input
                type="text"
                value={importForm.title}
                onChange={(event) =>
                  setImportForm((current) => ({
                    ...current,
                    title: event.target.value,
                  }))
                }
                placeholder="Title"
              />
              <input
                type="text"
                value={importForm.source}
                onChange={(event) =>
                  setImportForm((current) => ({
                    ...current,
                    source: event.target.value,
                  }))
                }
                placeholder={importModeCopy.sourcePlaceholder}
                required={importForm.mode === "file" || importForm.mode === "url"}
              />
              <textarea
                value={importForm.preview}
                onChange={(event) =>
                  setImportForm((current) => ({
                    ...current,
                    preview: event.target.value,
                  }))
                }
                placeholder={importModeCopy.previewPlaceholder}
              />
            </>
          )}
          {isUrlMode ? (
            <div className="knowledge-import-actions">
              <button
                type="button"
                className="knowledge-secondary-button"
                disabled={
                  fetchingPreview || importing || !importForm.source.trim()
                }
                onClick={() => void handleFetchUrlPreview()}
              >
                {fetchingPreview ? "Fetching preview..." : "Fetch URL preview"}
              </button>
              <button
                type="submit"
                disabled={importing || fetchingPreview || !importForm.missionId}
              >
                {importing ? "Importing..." : importModeCopy.button}
              </button>
            </div>
          ) : (
            !isFolderMode ? (
              <button type="submit" disabled={importing || !importForm.missionId}>
                {importing ? "Importing..." : importModeCopy.button}
              </button>
            ) : null
          )}
          {isUrlMode && fetchedPreviewMeta ? (
            <div className="knowledge-import-note">
              Preview fetched · HTTP {fetchedPreviewMeta.status}
              {fetchedPreviewMeta.contentType
                ? ` · ${fetchedPreviewMeta.contentType}`
                : ""}
              {fetchedPreviewMeta.truncated ? " · truncated" : ""}
              {` · ${fetchedPreviewMeta.fetchedAt}`}
            </div>
          ) : null}
          {importSummary ? (
            <div className="knowledge-import-success">{importSummary}</div>
          ) : null}
        </form>
      </section>
      {loading ? <div className="knowledge-empty">加载中...</div> : null}
      {!loading && items.length === 0 && sources.length === 0 ? (
        <div className="knowledge-empty">
          当前没有可检索的知识条目，也没有已持久化的知识源。
        </div>
      ) : null}

      {!loading && (items.length > 0 || sources.length > 0) ? (
        <div className="knowledge-layout">
          <section className="knowledge-sources">
            <div className="knowledge-panel-header">
              <div>
                <h3>Source List</h3>
                <p>真实持久化到 `knowledge_sources` 的导入记录。</p>
              </div>
              <span>{sources.length}</span>
            </div>
            {sources.length === 0 ? (
              <div className="knowledge-panel-empty">
                当前筛选条件下没有持久化知识源。
              </div>
            ) : (
              sources.map((source) => (
                <article key={source.id} className="knowledge-source-card">
                  <div className="knowledge-source-topline">
                    <span className="knowledge-item-kind">{source.type}</span>
                    <span className="knowledge-source-status">
                      {source.index_status}
                    </span>
                  </div>
                  <strong className="knowledge-source-title">
                    {source.title}
                  </strong>
                  <span className="knowledge-source-meta">
                    {source.chunk_count} chunks · {source.updated_at}
                  </span>
                  <span className="knowledge-source-uri">
                    {source.source_uri}
                  </span>
                </article>
              ))
            )}
          </section>

          <section className="knowledge-list">
            <div className="knowledge-panel-header">
              <div>
                <h3>Knowledge Feed</h3>
                <p>Mission context items 和 artifacts 的聚合视图。</p>
              </div>
              <span>{items.length}</span>
            </div>
            {items.length === 0 ? (
              <div className="knowledge-panel-empty">
                当前筛选条件下没有 knowledge feed 条目。
              </div>
            ) : (
              items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={`knowledge-item ${selectedId === item.id ? "knowledge-item-active" : ""}`}
                  onClick={() => setSelectedId(item.id)}
                >
                  <span className="knowledge-item-kind">
                    {item.source_kind}
                  </span>
                  <span className="knowledge-item-title">{item.title}</span>
                  <span className="knowledge-item-meta">
                    {item.mission_title} · {item.item_type}
                  </span>
                </button>
              ))
            )}
          </section>

          <section className="knowledge-preview">
            {selectedItem ? (
              <>
                <div className="knowledge-preview-eyebrow">
                  {selectedItem.source_kind}
                </div>
                <h3>{selectedItem.title}</h3>
                <div className="knowledge-preview-meta">
                  {selectedItem.mission_title} · {selectedItem.item_type} ·{" "}
                  {selectedItem.created_at}
                </div>
                <div className="knowledge-preview-block">
                  <span>Preview</span>
                  <strong>{selectedItem.preview ?? "-"}</strong>
                </div>
                <div className="knowledge-preview-block">
                  <span>Source</span>
                  <strong>{selectedItem.source ?? "-"}</strong>
                </div>
                <div className="knowledge-preview-block">
                  <span>Path</span>
                  <strong>{selectedItem.path ?? "-"}</strong>
                </div>
              </>
            ) : (
              <div className="knowledge-panel-empty">
                选择一条 Knowledge Feed 记录以查看详情。
              </div>
            )}
          </section>
        </div>
      ) : null}
    </div>
  );
}
