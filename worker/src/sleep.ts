import { Config } from "./config";

export const Sleep = async (seconds: number) => {
  return new Promise((resolve) => setTimeout(resolve, seconds * 1000));
};

export const SleepOnError = async () => {
    await Sleep(Config.waitOnErrorSec);
}

export const SleepCoolDown = async () => {
    await Sleep(Config.coolDownSec);
}

export const SleepOnIdle = async () => {
    await Sleep(Config.idleSleepSec);
}

export const SleepUpdateScores = async () => {
    await Sleep(15 * 60);
}
