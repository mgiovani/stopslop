function risky(): void {}
function doWork(): void {}

function EmptyCatch() {
  try {
    risky();
  } catch (e) {} // expect: SLOP005
  return null;
}

function LogOnlyCatch() {
  try {
    doWork();
  } catch (err) { // expect: SLOP005
    console.error(err);
  }
  return null;
}
