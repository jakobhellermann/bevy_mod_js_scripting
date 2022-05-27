function print(val) {
    Deno.core.print(val.toString() + "\n");
}

let i = 0;
function run() {
    i += 1;
    if (i % 100 == 0) {
        print(i);
    }
}
