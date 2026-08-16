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

export function compareVersions(current: string, latest: string): boolean {
  const cleanCurrent = current.replace(/^v/, "");
  const cleanLatest = latest.replace(/^v/, "");

  const currentParts = cleanCurrent.split(".").map(Number);
  const latestParts = cleanLatest.split(".").map(Number);

  for (let i = 0; i < Math.max(currentParts.length, latestParts.length); i++) {
    const currentPart = currentParts[i] || 0;
    const latestPart = latestParts[i] || 0;

    if (latestPart > currentPart) return true;
    if (latestPart < currentPart) return false;
  }

  return false;
}