import prisma from "./PrismaWibble";

class ModelSelector {
  private configuredModel(): string {
    return (
      process.env.LANGUAGE_MODEL ??
      process.env.OPENAI_MODEL ??
      process.env.TEXT_MODEL ??
      "gpt-3.5-turbo"
    )
      .split(",")[0]
      .trim();
  }

  async getNextGpt4Run(): Promise<Date | undefined> {
    return (
      await prisma.generation_schedule.findUnique({
        where: {
          id: "gpt4",
        },
      })
    )?.next_run;
  }

  async setNextGpt4Run(): Promise<void> {
    const intervalMs =
      parseInt(process.env.TIME_GPT4_HOURS ?? "8", 10) * 60 * 60 * 1000;
    const now = new Date().getTime();
    const nextRun = new Date(now + intervalMs);
    await prisma.generation_schedule.upsert({
      where: {
        id: "gpt4",
      },
      update: {
        next_run: nextRun,
      },
      create: {
        id: "gpt4",
        next_run: nextRun,
      },
    });
  }

  async selectNextModel(): Promise<string> {
    return this.configuredModel();
  }
}

const modelSelector = new ModelSelector();

export default modelSelector;
