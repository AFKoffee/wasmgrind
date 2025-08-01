# Browser Demo
This demo shows how to use Wasmgrind on the web as an npm package. 

## Prerequisites
This demo assumes that `npm` is installed on the system. We recommend to use the [node version manager (nvm)](https://github.com/nvm-sh/nvm) to ensure consistent behavior.

Because the _wasmgrind-js_ package is not yet published to the npm registry. You have to link the package locally by executing 

    npm link ../../crates/wasmgrind-js/pkg

inside the browser demo directory (the directory where this README is also located).

**Note:** This assumes you have built the wasmgrind-js package. If you haven't, refer to [wasmgrind-js](../../crates/wasmgrind-js/README.md) for build instructions.

## Running the Dev-Server
To run this demo using a webpack dev server, use

    npm run start

## Building the Demo
To bundle the demo into a single output directory, whose contents can be served by a custom web server, use

    npm run build