"use client";

const GITEE_API_TOKEN = "9957e6fc1011498ba0fd0602d37c9da0";
const GITEE_OWNER = "muliuawa";
const GITEE_REPO = "nexbox";

export interface GiteeRelease {
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

export async function fetchLatestRelease(): Promise<GiteeRelease | null> {
  try {
    const response = await fetch(
      `https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}/releases/latest`,
      {
        headers: {
          Authorization: `token ${GITEE_API_TOKEN}`,
          "Content-Type": "application/json",
        },
      }
    );

    if (!response.ok) {
      return null;
    }

    return await response.json();
  } catch (error) {
    return null;
  }
}

export async function fetchAllReleases(): Promise<GiteeRelease[]> {
  try {
    const response = await fetch(
      `https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}/releases?per_page=100`,
      {
        headers: {
          Authorization: `token ${GITEE_API_TOKEN}`,
          "Content-Type": "application/json",
        },
      }
    );

    if (!response.ok) {
      return [];
    }

    return await response.json();
  } catch (error) {
    return [];
  }
}

export async function fetchReleaseByTag(tag: string): Promise<GiteeRelease | null> {
  try {
    const response = await fetch(
      `https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}/releases/tags/${tag}`,
      {
        headers: {
          Authorization: `token ${GITEE_API_TOKEN}`,
          "Content-Type": "application/json",
        },
      }
    );

    if (!response.ok) {
      return null;
    }

    return await response.json();
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
