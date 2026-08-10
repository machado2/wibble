import { DateTime } from "luxon";

export const getHumanReadableDate = (isoDate: string) => {
  // if the date part is 07/01 we return 06/31
  // as explained on
  // https://wibble.fbmac.net/content/april-fools-day-moved-to-june-31
  const date = DateTime.fromISO(isoDate);
  if (date.month === 7 && date.day === 1) {
    const date30jun = date.minus({ days: 1 });
    const human30jun = date30jun.toLocaleString(DateTime.DATETIME_MED);
    return human30jun.replace("30", "31");
  } else {
    return date.toLocaleString(DateTime.DATETIME_MED);
  }
};
