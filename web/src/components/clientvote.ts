const postVote = (url: string, body: Record<string, unknown>) =>
  fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });

export const upvote = (contentId: string) =>
  postVote("/api/vote", { content_id: contentId });

export const unvote = (contentId: string) =>
  postVote("/api/unvote", { content_id: contentId });

export const downvote = (contentId: string) =>
  postVote("/api/vote", { content_id: contentId, downvote: true });
