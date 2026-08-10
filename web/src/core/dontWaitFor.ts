export function dontWaitFor<T>(promise: Promise<T>) {
  promise.catch((e) => {
    console.error(e);
  });
}
