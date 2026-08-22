export type NewsListItem = {
  id: string;
  title: string;
  description: string;
  imagePrompt: string;
  slug: string;
  created_at: string;
  votes: number;
  currentVote: number;
  hotScore: number;
  translationState?: "translated" | "pending" | "failed";
};
