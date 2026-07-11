function risky(): void {}
function doWork(): void {}

function emptyCatch() {
  try {
    risky();
  } catch (e) {} // expect: SLOP005
}

function logOnlyCatch() {
  try {
    doWork();
  } catch (err) { // expect: SLOP005
    console.log(err);
  }
}

function commentOnlyCatch() {
  try {
    doWork();
  } catch (e) { // expect: SLOP005
    // left blank
  }
}

function multiConsoleCatch() {
  try {
    doWork();
  } catch (e) { // expect: SLOP005
    console.warn(e);
    console.error(e);
  }
}
