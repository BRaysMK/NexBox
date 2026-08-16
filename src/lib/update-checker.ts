"use client";

// 个人定制版更新源：指向本 Fork 的 GitHub Release（BRaysMK/NexBox）
// 若需切回上游，将 OWNER/REPO 改回 MuLiuSaMa/nexbox 并换用 GitCode API。
const GH_OWNER = "BRaysMK";
const GH_REPO = "NexBox";
const GH_WEB = "https://github.com";

export interface ReleaseInfo {
  tag_name: string;
  name: string;
  body: string;
  assets: Array<{
    name: string;
    browser_download_url: string;
    size: number;
  }>;
  published_at: string;
  html_url: string;
}

const releaseBaseUrl = (path: string) =>
  `https://api.github.com/repos/${GH_OWNER}/${GH_REPO}${path}`;

// GitHub 的 release 响应自带 html_url；无则按仓库与 tag 拼接
function releaseHtmlUrl(tagName: string): string {
  return `${GH_WEB}/${GH_OWNER}/${GH_REPO}/releases/tag/${tagName}`;
}

export async function fetchLatestRelease(): Promise<ReleaseInfo | null> {
  try {
    const response = await fetch(releaseBaseUrl("/releases/latest"), {
      headers: { "Accept": "application/vnd.github+json", "User-Agent": "NexBox" },
    });

    if (!response.ok) {
      return null;
    }

    const data = await response.json();
    return { ...data, html_url: data.html_url || releaseHtmlUrl(data.tag_name) };
  } catch (error) {
    return null;
  }
}

export async function fetchAllReleases(): Promise<ReleaseInfo[]> {
  try {
    const response = await fetch(releaseBaseUrl("/releases?per_page=100"), {
      headers: { "Accept": "application/vnd.github+json", "User-Agent": "NexBox" },
    });

    if (!response.ok) {
      return [];
    }

    const data = await response.json();
    return (data as ReleaseInfo[]).map((r) => ({
      ...r,
      html_url: r.html_url || releaseHtmlUrl(r.tag_name),
    }));
  } catch (error) {
    return [];
  }
}

export async function fetchReleaseByTag(tag: string): Promise<ReleaseInfo | null> {
  try {
    const response = await fetch(
      releaseBaseUrl(`/releases/tags/${encodeURIComponent(tag)}`),
      {
        headers: { "Accept": "application/vnd.github+json", "User-Agent": "NexBox" },
      }
    );

    if (!response.ok) {
      return null;
    }

    const data = await response.json();
    return { ...data, html_url: data.html_url || releaseHtmlUrl(data.tag_name) };
  } catch (error) {
    return null;
  }
}

// 解析版本为数字段数组，支持 8.0.9 和 8.0.9+08161600（构建元数据月日时分后缀）两种格式。
// 返回 [major, minor, patch, build]（build 是 + 后的数字部分，无则为 0）。
// 也兼容旧的 - 预发布格式（8.0.9-08161600）。
//
// 日期后缀对齐为 8 位数字（MMDDHHMM）：0816 → 08160000，08161600 → 08161600。
// 这样同一构建内时间递增可正确比较。注意：构建元数据在前导 0 上比预发布宽松（semver 允许），
// 因此 08161600 这种带前导 0 的日期格式必须用 + 而非 -。
function parseVersion(v: string): number[] {
  const clean = v.replace(/^v/, "").trim();
  const sep = clean.includes("+") ? "+" : "-";
  const [core, meta] = clean.split(sep);
  const parts = core.split(".").map((p) => {
    const n = parseInt(p, 10);
    return Number.isNaN(n) ? 0 : n;
  });
  while (parts.length < 3) parts.push(0);
  // 日期后缀：取分隔符后第一个数字串，右补 0 到 8 位后比较
  let metaNum = 0;
  if (meta) {
    const metaNumStr = meta.split(".")[0].match(/\d+/);
    if (metaNumStr) {
      const raw = metaNumStr[0];
      metaNum = parseInt(raw.padEnd(8, "0"), 10);
    }
  }
  parts.push(metaNum);
  return parts;
}

export function compareVersions(current: string, latest: string): boolean {
  const c = parseVersion(current);
  const l = parseVersion(latest);
  const len = Math.max(c.length, l.length);
  for (let i = 0; i < len; i++) {
    const cv = c[i] || 0;
    const lv = l[i] || 0;
    if (lv > cv) return true;
    if (lv < cv) return false;
  }
  return false;
}