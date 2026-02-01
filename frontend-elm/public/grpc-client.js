(() => {
  // node_modules/@protobuf-ts/runtime/build/es2015/base64.js
  var encTable = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".split("");
  var decTable = [];
  for (let i = 0; i < encTable.length; i++)
    decTable[encTable[i].charCodeAt(0)] = i;
  decTable["-".charCodeAt(0)] = encTable.indexOf("+");
  decTable["_".charCodeAt(0)] = encTable.indexOf("/");
  function base64decode(base64Str) {
    let es = base64Str.length * 3 / 4;
    if (base64Str[base64Str.length - 2] == "=")
      es -= 2;
    else if (base64Str[base64Str.length - 1] == "=")
      es -= 1;
    let bytes = new Uint8Array(es), bytePos = 0, groupPos = 0, b, p = 0;
    for (let i = 0; i < base64Str.length; i++) {
      b = decTable[base64Str.charCodeAt(i)];
      if (b === void 0) {
        switch (base64Str[i]) {
          case "=":
            groupPos = 0;
          case "\n":
          case "\r":
          case "	":
          case " ":
            continue;
          default:
            throw Error(`invalid base64 string.`);
        }
      }
      switch (groupPos) {
        case 0:
          p = b;
          groupPos = 1;
          break;
        case 1:
          bytes[bytePos++] = p << 2 | (b & 48) >> 4;
          p = b;
          groupPos = 2;
          break;
        case 2:
          bytes[bytePos++] = (p & 15) << 4 | (b & 60) >> 2;
          p = b;
          groupPos = 3;
          break;
        case 3:
          bytes[bytePos++] = (p & 3) << 6 | b;
          groupPos = 0;
          break;
      }
    }
    if (groupPos == 1)
      throw Error(`invalid base64 string.`);
    return bytes.subarray(0, bytePos);
  }
  function base64encode(bytes) {
    let base64 = "", groupPos = 0, b, p = 0;
    for (let i = 0; i < bytes.length; i++) {
      b = bytes[i];
      switch (groupPos) {
        case 0:
          base64 += encTable[b >> 2];
          p = (b & 3) << 4;
          groupPos = 1;
          break;
        case 1:
          base64 += encTable[p | b >> 4];
          p = (b & 15) << 2;
          groupPos = 2;
          break;
        case 2:
          base64 += encTable[p | b >> 6];
          base64 += encTable[b & 63];
          groupPos = 0;
          break;
      }
    }
    if (groupPos) {
      base64 += encTable[p];
      base64 += "=";
      if (groupPos == 1)
        base64 += "=";
    }
    return base64;
  }

  // node_modules/@protobuf-ts/runtime/build/es2015/binary-format-contract.js
  var UnknownFieldHandler;
  (function(UnknownFieldHandler3) {
    UnknownFieldHandler3.symbol = Symbol.for("protobuf-ts/unknown");
    UnknownFieldHandler3.onRead = (typeName, message, fieldNo, wireType, data) => {
      let container = is(message) ? message[UnknownFieldHandler3.symbol] : message[UnknownFieldHandler3.symbol] = [];
      container.push({ no: fieldNo, wireType, data });
    };
    UnknownFieldHandler3.onWrite = (typeName, message, writer) => {
      for (let { no, wireType, data } of UnknownFieldHandler3.list(message))
        writer.tag(no, wireType).raw(data);
    };
    UnknownFieldHandler3.list = (message, fieldNo) => {
      if (is(message)) {
        let all = message[UnknownFieldHandler3.symbol];
        return fieldNo ? all.filter((uf) => uf.no == fieldNo) : all;
      }
      return [];
    };
    UnknownFieldHandler3.last = (message, fieldNo) => UnknownFieldHandler3.list(message, fieldNo).slice(-1)[0];
    const is = (message) => message && Array.isArray(message[UnknownFieldHandler3.symbol]);
  })(UnknownFieldHandler || (UnknownFieldHandler = {}));
  function mergeBinaryOptions(a, b) {
    return Object.assign(Object.assign({}, a), b);
  }
  var WireType;
  (function(WireType3) {
    WireType3[WireType3["Varint"] = 0] = "Varint";
    WireType3[WireType3["Bit64"] = 1] = "Bit64";
    WireType3[WireType3["LengthDelimited"] = 2] = "LengthDelimited";
    WireType3[WireType3["StartGroup"] = 3] = "StartGroup";
    WireType3[WireType3["EndGroup"] = 4] = "EndGroup";
    WireType3[WireType3["Bit32"] = 5] = "Bit32";
  })(WireType || (WireType = {}));

  // node_modules/@protobuf-ts/runtime/build/es2015/assert.js
  function assert(condition, msg) {
    if (!condition) {
      throw new Error(msg);
    }
  }

  // node_modules/@protobuf-ts/runtime/build/es2015/json-format-contract.js
  function mergeJsonOptions(a, b) {
    var _a, _b;
    let c = Object.assign(Object.assign({}, a), b);
    c.typeRegistry = [...(_a = a === null || a === void 0 ? void 0 : a.typeRegistry) !== null && _a !== void 0 ? _a : [], ...(_b = b === null || b === void 0 ? void 0 : b.typeRegistry) !== null && _b !== void 0 ? _b : []];
    return c;
  }

  // node_modules/@protobuf-ts/runtime-rpc/build/es2015/rpc-error.js
  var RpcError = class extends Error {
    constructor(message, code = "UNKNOWN", meta) {
      super(message);
      this.name = "RpcError";
      Object.setPrototypeOf(this, new.target.prototype);
      this.code = code;
      this.meta = meta !== null && meta !== void 0 ? meta : {};
    }
    toString() {
      const l = [this.name + ": " + this.message];
      if (this.code) {
        l.push("");
        l.push("Code: " + this.code);
      }
      if (this.serviceName && this.methodName) {
        l.push("Method: " + this.serviceName + "/" + this.methodName);
      }
      let m = Object.entries(this.meta);
      if (m.length) {
        l.push("");
        l.push("Meta:");
        for (let [k, v] of m) {
          l.push(`  ${k}: ${v}`);
        }
      }
      return l.join("\n");
    }
  };

  // node_modules/@protobuf-ts/runtime-rpc/build/es2015/rpc-options.js
  function mergeRpcOptions(defaults, options) {
    if (!options)
      return defaults;
    let o = {};
    copy(defaults, o);
    copy(options, o);
    for (let key of Object.keys(options)) {
      let val = options[key];
      switch (key) {
        case "jsonOptions":
          o.jsonOptions = mergeJsonOptions(defaults.jsonOptions, o.jsonOptions);
          break;
        case "binaryOptions":
          o.binaryOptions = mergeBinaryOptions(defaults.binaryOptions, o.binaryOptions);
          break;
        case "meta":
          o.meta = {};
          copy(defaults.meta, o.meta);
          copy(options.meta, o.meta);
          break;
        case "interceptors":
          o.interceptors = defaults.interceptors ? defaults.interceptors.concat(val) : val.concat();
          break;
      }
    }
    return o;
  }
  function copy(a, into) {
    if (!a)
      return;
    let c = into;
    for (let [k, v] of Object.entries(a)) {
      if (v instanceof Date)
        c[k] = new Date(v.getTime());
      else if (Array.isArray(v))
        c[k] = v.concat();
      else
        c[k] = v;
    }
  }

  // node_modules/@protobuf-ts/runtime-rpc/build/es2015/deferred.js
  var DeferredState;
  (function(DeferredState2) {
    DeferredState2[DeferredState2["PENDING"] = 0] = "PENDING";
    DeferredState2[DeferredState2["REJECTED"] = 1] = "REJECTED";
    DeferredState2[DeferredState2["RESOLVED"] = 2] = "RESOLVED";
  })(DeferredState || (DeferredState = {}));
  var Deferred = class {
    /**
     * @param preventUnhandledRejectionWarning - prevents the warning
     * "Unhandled Promise rejection" by adding a noop rejection handler.
     * Working with calls returned from the runtime-rpc package in an
     * async function usually means awaiting one call property after
     * the other. This means that the "status" is not being awaited when
     * an earlier await for the "headers" is rejected. This causes the
     * "unhandled promise reject" warning. A more correct behaviour for
     * calls might be to become aware whether at least one of the
     * promises is handled and swallow the rejection warning for the
     * others.
     */
    constructor(preventUnhandledRejectionWarning = true) {
      this._state = DeferredState.PENDING;
      this._promise = new Promise((resolve, reject) => {
        this._resolve = resolve;
        this._reject = reject;
      });
      if (preventUnhandledRejectionWarning) {
        this._promise.catch((_) => {
        });
      }
    }
    /**
     * Get the current state of the promise.
     */
    get state() {
      return this._state;
    }
    /**
     * Get the deferred promise.
     */
    get promise() {
      return this._promise;
    }
    /**
     * Resolve the promise. Throws if the promise is already resolved or rejected.
     */
    resolve(value) {
      if (this.state !== DeferredState.PENDING)
        throw new Error(`cannot resolve ${DeferredState[this.state].toLowerCase()}`);
      this._resolve(value);
      this._state = DeferredState.RESOLVED;
    }
    /**
     * Reject the promise. Throws if the promise is already resolved or rejected.
     */
    reject(reason) {
      if (this.state !== DeferredState.PENDING)
        throw new Error(`cannot reject ${DeferredState[this.state].toLowerCase()}`);
      this._reject(reason);
      this._state = DeferredState.REJECTED;
    }
    /**
     * Resolve the promise. Ignore if not pending.
     */
    resolvePending(val) {
      if (this._state === DeferredState.PENDING)
        this.resolve(val);
    }
    /**
     * Reject the promise. Ignore if not pending.
     */
    rejectPending(reason) {
      if (this._state === DeferredState.PENDING)
        this.reject(reason);
    }
  };

  // node_modules/@protobuf-ts/runtime-rpc/build/es2015/rpc-output-stream.js
  var RpcOutputStreamController = class {
    constructor() {
      this._lis = {
        nxt: [],
        msg: [],
        err: [],
        cmp: []
      };
      this._closed = false;
      this._itState = { q: [] };
    }
    // --- RpcOutputStream callback API
    onNext(callback) {
      return this.addLis(callback, this._lis.nxt);
    }
    onMessage(callback) {
      return this.addLis(callback, this._lis.msg);
    }
    onError(callback) {
      return this.addLis(callback, this._lis.err);
    }
    onComplete(callback) {
      return this.addLis(callback, this._lis.cmp);
    }
    addLis(callback, list) {
      list.push(callback);
      return () => {
        let i = list.indexOf(callback);
        if (i >= 0)
          list.splice(i, 1);
      };
    }
    // remove all listeners
    clearLis() {
      for (let l of Object.values(this._lis))
        l.splice(0, l.length);
    }
    // --- Controller API
    /**
     * Is this stream already closed by a completion or error?
     */
    get closed() {
      return this._closed !== false;
    }
    /**
     * Emit message, close with error, or close successfully, but only one
     * at a time.
     * Can be used to wrap a stream by using the other stream's `onNext`.
     */
    notifyNext(message, error, complete) {
      assert((message ? 1 : 0) + (error ? 1 : 0) + (complete ? 1 : 0) <= 1, "only one emission at a time");
      if (message)
        this.notifyMessage(message);
      if (error)
        this.notifyError(error);
      if (complete)
        this.notifyComplete();
    }
    /**
     * Emits a new message. Throws if stream is closed.
     *
     * Triggers onNext and onMessage callbacks.
     */
    notifyMessage(message) {
      assert(!this.closed, "stream is closed");
      this.pushIt({ value: message, done: false });
      this._lis.msg.forEach((l) => l(message));
      this._lis.nxt.forEach((l) => l(message, void 0, false));
    }
    /**
     * Closes the stream with an error. Throws if stream is closed.
     *
     * Triggers onNext and onError callbacks.
     */
    notifyError(error) {
      assert(!this.closed, "stream is closed");
      this._closed = error;
      this.pushIt(error);
      this._lis.err.forEach((l) => l(error));
      this._lis.nxt.forEach((l) => l(void 0, error, false));
      this.clearLis();
    }
    /**
     * Closes the stream successfully. Throws if stream is closed.
     *
     * Triggers onNext and onComplete callbacks.
     */
    notifyComplete() {
      assert(!this.closed, "stream is closed");
      this._closed = true;
      this.pushIt({ value: null, done: true });
      this._lis.cmp.forEach((l) => l());
      this._lis.nxt.forEach((l) => l(void 0, void 0, true));
      this.clearLis();
    }
    /**
     * Creates an async iterator (that can be used with `for await {...}`)
     * to consume the stream.
     *
     * Some things to note:
     * - If an error occurs, the `for await` will throw it.
     * - If an error occurred before the `for await` was started, `for await`
     *   will re-throw it.
     * - If the stream is already complete, the `for await` will be empty.
     * - If your `for await` consumes slower than the stream produces,
     *   for example because you are relaying messages in a slow operation,
     *   messages are queued.
     */
    [Symbol.asyncIterator]() {
      if (this._closed === true)
        this.pushIt({ value: null, done: true });
      else if (this._closed !== false)
        this.pushIt(this._closed);
      return {
        next: () => {
          let state = this._itState;
          assert(state, "bad state");
          assert(!state.p, "iterator contract broken");
          let first = state.q.shift();
          if (first)
            return "value" in first ? Promise.resolve(first) : Promise.reject(first);
          state.p = new Deferred();
          return state.p.promise;
        }
      };
    }
    // "push" a new iterator result.
    // this either resolves a pending promise, or enqueues the result.
    pushIt(result) {
      let state = this._itState;
      if (state.p) {
        const p = state.p;
        assert(p.state == DeferredState.PENDING, "iterator contract broken");
        "value" in result ? p.resolve(result) : p.reject(result);
        delete state.p;
      } else {
        state.q.push(result);
      }
    }
  };

  // node_modules/@protobuf-ts/runtime-rpc/build/es2015/unary-call.js
  var __awaiter = function(thisArg, _arguments, P, generator) {
    function adopt(value) {
      return value instanceof P ? value : new P(function(resolve) {
        resolve(value);
      });
    }
    return new (P || (P = Promise))(function(resolve, reject) {
      function fulfilled(value) {
        try {
          step(generator.next(value));
        } catch (e) {
          reject(e);
        }
      }
      function rejected(value) {
        try {
          step(generator["throw"](value));
        } catch (e) {
          reject(e);
        }
      }
      function step(result) {
        result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected);
      }
      step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
  };
  var UnaryCall = class {
    constructor(method, requestHeaders, request, headers, response, status, trailers) {
      this.method = method;
      this.requestHeaders = requestHeaders;
      this.request = request;
      this.headers = headers;
      this.response = response;
      this.status = status;
      this.trailers = trailers;
    }
    /**
     * If you are only interested in the final outcome of this call,
     * you can await it to receive a `FinishedUnaryCall`.
     */
    then(onfulfilled, onrejected) {
      return this.promiseFinished().then((value) => onfulfilled ? Promise.resolve(onfulfilled(value)) : value, (reason) => onrejected ? Promise.resolve(onrejected(reason)) : Promise.reject(reason));
    }
    promiseFinished() {
      return __awaiter(this, void 0, void 0, function* () {
        let [headers, response, status, trailers] = yield Promise.all([this.headers, this.response, this.status, this.trailers]);
        return {
          method: this.method,
          requestHeaders: this.requestHeaders,
          request: this.request,
          headers,
          response,
          status,
          trailers
        };
      });
    }
  };

  // node_modules/@protobuf-ts/runtime-rpc/build/es2015/server-streaming-call.js
  var __awaiter2 = function(thisArg, _arguments, P, generator) {
    function adopt(value) {
      return value instanceof P ? value : new P(function(resolve) {
        resolve(value);
      });
    }
    return new (P || (P = Promise))(function(resolve, reject) {
      function fulfilled(value) {
        try {
          step(generator.next(value));
        } catch (e) {
          reject(e);
        }
      }
      function rejected(value) {
        try {
          step(generator["throw"](value));
        } catch (e) {
          reject(e);
        }
      }
      function step(result) {
        result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected);
      }
      step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
  };
  var ServerStreamingCall = class {
    constructor(method, requestHeaders, request, headers, response, status, trailers) {
      this.method = method;
      this.requestHeaders = requestHeaders;
      this.request = request;
      this.headers = headers;
      this.responses = response;
      this.status = status;
      this.trailers = trailers;
    }
    /**
     * Instead of awaiting the response status and trailers, you can
     * just as well await this call itself to receive the server outcome.
     * You should first setup some listeners to the `request` to
     * see the actual messages the server replied with.
     */
    then(onfulfilled, onrejected) {
      return this.promiseFinished().then((value) => onfulfilled ? Promise.resolve(onfulfilled(value)) : value, (reason) => onrejected ? Promise.resolve(onrejected(reason)) : Promise.reject(reason));
    }
    promiseFinished() {
      return __awaiter2(this, void 0, void 0, function* () {
        let [headers, status, trailers] = yield Promise.all([this.headers, this.status, this.trailers]);
        return {
          method: this.method,
          requestHeaders: this.requestHeaders,
          request: this.request,
          headers,
          status,
          trailers
        };
      });
    }
  };

  // node_modules/@protobuf-ts/grpcweb-transport/build/es2015/goog-grpc-status-code.js
  var GrpcStatusCode;
  (function(GrpcStatusCode2) {
    GrpcStatusCode2[GrpcStatusCode2["OK"] = 0] = "OK";
    GrpcStatusCode2[GrpcStatusCode2["CANCELLED"] = 1] = "CANCELLED";
    GrpcStatusCode2[GrpcStatusCode2["UNKNOWN"] = 2] = "UNKNOWN";
    GrpcStatusCode2[GrpcStatusCode2["INVALID_ARGUMENT"] = 3] = "INVALID_ARGUMENT";
    GrpcStatusCode2[GrpcStatusCode2["DEADLINE_EXCEEDED"] = 4] = "DEADLINE_EXCEEDED";
    GrpcStatusCode2[GrpcStatusCode2["NOT_FOUND"] = 5] = "NOT_FOUND";
    GrpcStatusCode2[GrpcStatusCode2["ALREADY_EXISTS"] = 6] = "ALREADY_EXISTS";
    GrpcStatusCode2[GrpcStatusCode2["PERMISSION_DENIED"] = 7] = "PERMISSION_DENIED";
    GrpcStatusCode2[GrpcStatusCode2["UNAUTHENTICATED"] = 16] = "UNAUTHENTICATED";
    GrpcStatusCode2[GrpcStatusCode2["RESOURCE_EXHAUSTED"] = 8] = "RESOURCE_EXHAUSTED";
    GrpcStatusCode2[GrpcStatusCode2["FAILED_PRECONDITION"] = 9] = "FAILED_PRECONDITION";
    GrpcStatusCode2[GrpcStatusCode2["ABORTED"] = 10] = "ABORTED";
    GrpcStatusCode2[GrpcStatusCode2["OUT_OF_RANGE"] = 11] = "OUT_OF_RANGE";
    GrpcStatusCode2[GrpcStatusCode2["UNIMPLEMENTED"] = 12] = "UNIMPLEMENTED";
    GrpcStatusCode2[GrpcStatusCode2["INTERNAL"] = 13] = "INTERNAL";
    GrpcStatusCode2[GrpcStatusCode2["UNAVAILABLE"] = 14] = "UNAVAILABLE";
    GrpcStatusCode2[GrpcStatusCode2["DATA_LOSS"] = 15] = "DATA_LOSS";
  })(GrpcStatusCode || (GrpcStatusCode = {}));

  // node_modules/@protobuf-ts/grpcweb-transport/build/es2015/grpc-web-format.js
  var __awaiter3 = function(thisArg, _arguments, P, generator) {
    function adopt(value) {
      return value instanceof P ? value : new P(function(resolve) {
        resolve(value);
      });
    }
    return new (P || (P = Promise))(function(resolve, reject) {
      function fulfilled(value) {
        try {
          step(generator.next(value));
        } catch (e) {
          reject(e);
        }
      }
      function rejected(value) {
        try {
          step(generator["throw"](value));
        } catch (e) {
          reject(e);
        }
      }
      function step(result) {
        result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected);
      }
      step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
  };
  function createGrpcWebRequestHeader(headers, format, timeout, meta, userAgent) {
    if (meta) {
      for (let [k, v] of Object.entries(meta)) {
        if (typeof v == "string")
          headers.append(k, v);
        else
          for (let i of v)
            headers.append(k, i);
      }
    }
    headers.set("Content-Type", format === "text" ? "application/grpc-web-text" : "application/grpc-web+proto");
    if (format == "text") {
      headers.set("Accept", "application/grpc-web-text");
    }
    headers.set("X-Grpc-Web", "1");
    if (userAgent)
      headers.set("X-User-Agent", userAgent);
    if (typeof timeout === "number") {
      if (timeout <= 0) {
        throw new RpcError(`timeout ${timeout} ms exceeded`, GrpcStatusCode[GrpcStatusCode.DEADLINE_EXCEEDED]);
      }
      headers.set("grpc-timeout", `${timeout}m`);
    } else if (timeout) {
      const deadline = timeout.getTime();
      const now = Date.now();
      if (deadline <= now) {
        throw new RpcError(`deadline ${timeout} exceeded`, GrpcStatusCode[GrpcStatusCode.DEADLINE_EXCEEDED]);
      }
      headers.set("grpc-timeout", `${deadline - now}m`);
    }
    return headers;
  }
  function createGrpcWebRequestBody(message, format) {
    let body = new Uint8Array(5 + message.length);
    body[0] = GrpcWebFrame.DATA;
    for (let msgLen = message.length, i = 4; i > 0; i--) {
      body[i] = msgLen % 256;
      msgLen >>>= 8;
    }
    body.set(message, 5);
    return format === "binary" ? body : base64encode(body);
  }
  function readGrpcWebResponseHeader(headersOrFetchResponse, httpStatus, httpStatusText) {
    if (arguments.length === 1) {
      let fetchResponse = headersOrFetchResponse;
      let responseType;
      try {
        responseType = fetchResponse.type;
      } catch (_a) {
      }
      switch (responseType) {
        case "error":
        case "opaque":
        case "opaqueredirect":
          throw new RpcError(`fetch response type ${fetchResponse.type}`, GrpcStatusCode[GrpcStatusCode.UNKNOWN]);
      }
      return readGrpcWebResponseHeader(fetchHeadersToHttp(fetchResponse.headers), fetchResponse.status, fetchResponse.statusText);
    }
    let headers = headersOrFetchResponse, httpOk = httpStatus >= 200 && httpStatus < 300, responseMeta = parseMetadata(headers), [statusCode, statusDetail] = parseStatus(headers);
    if ((statusCode === void 0 || statusCode === GrpcStatusCode.OK) && !httpOk) {
      statusCode = httpStatusToGrpc(httpStatus);
      statusDetail = httpStatusText;
    }
    return [statusCode, statusDetail, responseMeta];
  }
  function readGrpcWebResponseTrailer(data) {
    let headers = parseTrailer(data), [code, detail] = parseStatus(headers), meta = parseMetadata(headers);
    return [code !== null && code !== void 0 ? code : GrpcStatusCode.OK, detail, meta];
  }
  var GrpcWebFrame;
  (function(GrpcWebFrame2) {
    GrpcWebFrame2[GrpcWebFrame2["DATA"] = 0] = "DATA";
    GrpcWebFrame2[GrpcWebFrame2["TRAILER"] = 128] = "TRAILER";
  })(GrpcWebFrame || (GrpcWebFrame = {}));
  function readGrpcWebResponseBody(stream, contentType, onFrame) {
    return __awaiter3(this, void 0, void 0, function* () {
      let streamReader, base64queue = "", byteQueue = new Uint8Array(0), format = parseFormat(contentType);
      if (isReadableStream(stream)) {
        let whatWgReadableStream = stream.getReader();
        streamReader = {
          next: () => whatWgReadableStream.read()
        };
      } else {
        streamReader = stream[Symbol.asyncIterator]();
      }
      while (true) {
        let result = yield streamReader.next();
        if (result.value !== void 0) {
          if (format === "text") {
            for (let i = 0; i < result.value.length; i++)
              base64queue += String.fromCharCode(result.value[i]);
            let safeLen = base64queue.length - base64queue.length % 4;
            if (safeLen === 0)
              continue;
            byteQueue = concatBytes(byteQueue, base64decode(base64queue.substring(0, safeLen)));
            base64queue = base64queue.substring(safeLen);
          } else {
            byteQueue = concatBytes(byteQueue, result.value);
          }
          while (byteQueue.length >= 5 && byteQueue[0] === GrpcWebFrame.DATA) {
            let msgLen = 0;
            for (let i = 1; i < 5; i++)
              msgLen = (msgLen << 8) + byteQueue[i];
            if (byteQueue.length - 5 >= msgLen) {
              onFrame(GrpcWebFrame.DATA, byteQueue.subarray(5, 5 + msgLen));
              byteQueue = byteQueue.subarray(5 + msgLen);
            } else
              break;
          }
        }
        if (result.done) {
          if (byteQueue.length === 0)
            break;
          if (byteQueue[0] !== GrpcWebFrame.TRAILER || byteQueue.length < 5)
            throw new RpcError("premature EOF", GrpcStatusCode[GrpcStatusCode.DATA_LOSS]);
          onFrame(GrpcWebFrame.TRAILER, byteQueue.subarray(5));
          break;
        }
      }
    });
  }
  var isReadableStream = (s) => {
    return typeof s.getReader == "function";
  };
  function concatBytes(a, b) {
    let n = new Uint8Array(a.length + b.length);
    n.set(a);
    n.set(b, a.length);
    return n;
  }
  function parseFormat(contentType) {
    switch (contentType) {
      case "application/grpc-web-text":
      case "application/grpc-web-text+proto":
        return "text";
      case "application/grpc-web":
      case "application/grpc-web+proto":
        return "binary";
      case void 0:
      case null:
        throw new RpcError("missing response content type", GrpcStatusCode[GrpcStatusCode.INTERNAL]);
      default:
        throw new RpcError("unexpected response content type: " + contentType, GrpcStatusCode[GrpcStatusCode.INTERNAL]);
    }
  }
  function parseStatus(headers) {
    let code, message;
    let m = headers["grpc-message"];
    if (m !== void 0) {
      if (Array.isArray(m))
        return [GrpcStatusCode.INTERNAL, "invalid grpc-web message"];
      message = m;
    }
    let s = headers["grpc-status"];
    if (s !== void 0) {
      if (Array.isArray(s))
        return [GrpcStatusCode.INTERNAL, "invalid grpc-web status"];
      code = parseInt(s, 10);
      if (GrpcStatusCode[code] === void 0)
        return [GrpcStatusCode.INTERNAL, "invalid grpc-web status"];
    }
    return [code, message];
  }
  function parseMetadata(headers) {
    let meta = {};
    for (let [k, v] of Object.entries(headers))
      switch (k) {
        case "grpc-message":
        case "grpc-status":
        case "content-type":
          break;
        default:
          meta[k] = v;
      }
    return meta;
  }
  function parseTrailer(trailerData) {
    let headers = {};
    for (let chunk of String.fromCharCode.apply(String, trailerData).trim().split("\r\n")) {
      if (chunk == "")
        continue;
      let [key, ...val] = chunk.split(":");
      const value = val.join(":").trim();
      key = key.trim();
      let e = headers[key];
      if (typeof e == "string")
        headers[key] = [e, value];
      else if (Array.isArray(e))
        e.push(value);
      else
        headers[key] = value;
    }
    return headers;
  }
  function fetchHeadersToHttp(fetchHeaders) {
    let headers = {};
    fetchHeaders.forEach((value, key) => {
      let e = headers[key];
      if (typeof e == "string")
        headers[key] = [e, value];
      else if (Array.isArray(e))
        e.push(value);
      else
        headers[key] = value;
    });
    return headers;
  }
  function httpStatusToGrpc(httpStatus) {
    switch (httpStatus) {
      case 200:
        return GrpcStatusCode.OK;
      case 400:
        return GrpcStatusCode.INVALID_ARGUMENT;
      case 401:
        return GrpcStatusCode.UNAUTHENTICATED;
      case 403:
        return GrpcStatusCode.PERMISSION_DENIED;
      case 404:
        return GrpcStatusCode.NOT_FOUND;
      case 409:
        return GrpcStatusCode.ABORTED;
      case 412:
        return GrpcStatusCode.FAILED_PRECONDITION;
      case 429:
        return GrpcStatusCode.RESOURCE_EXHAUSTED;
      case 499:
        return GrpcStatusCode.CANCELLED;
      case 500:
        return GrpcStatusCode.UNKNOWN;
      case 501:
        return GrpcStatusCode.UNIMPLEMENTED;
      case 503:
        return GrpcStatusCode.UNAVAILABLE;
      case 504:
        return GrpcStatusCode.DEADLINE_EXCEEDED;
      default:
        return GrpcStatusCode.UNKNOWN;
    }
  }

  // node_modules/@protobuf-ts/grpcweb-transport/build/es2015/grpc-web-transport.js
  var GrpcWebFetchTransport = class {
    constructor(defaultOptions) {
      this.defaultOptions = defaultOptions;
    }
    mergeOptions(options) {
      return mergeRpcOptions(this.defaultOptions, options);
    }
    /**
     * Create an URI for a gRPC web call.
     *
     * Takes the `baseUrl` option and appends:
     * - slash "/"
     * - package name
     * - dot "."
     * - service name
     * - slash "/"
     * - method name
     *
     * If the service was declared without a package, the package name and dot
     * are omitted.
     *
     * All names are used exactly like declared in .proto.
     */
    makeUrl(method, options) {
      let base = options.baseUrl;
      if (base.endsWith("/"))
        base = base.substring(0, base.length - 1);
      return `${base}/${method.service.typeName}/${method.name}`;
    }
    clientStreaming(method) {
      const e = new RpcError("Client streaming is not supported by grpc-web", GrpcStatusCode[GrpcStatusCode.UNIMPLEMENTED]);
      e.methodName = method.name;
      e.serviceName = method.service.typeName;
      throw e;
    }
    duplex(method) {
      const e = new RpcError("Duplex streaming is not supported by grpc-web", GrpcStatusCode[GrpcStatusCode.UNIMPLEMENTED]);
      e.methodName = method.name;
      e.serviceName = method.service.typeName;
      throw e;
    }
    serverStreaming(method, input, options) {
      var _a, _b, _c, _d, _e;
      let opt = options, format = (_a = opt.format) !== null && _a !== void 0 ? _a : "text", fetch = (_b = opt.fetch) !== null && _b !== void 0 ? _b : globalThis.fetch, fetchInit = (_c = opt.fetchInit) !== null && _c !== void 0 ? _c : {}, url = this.makeUrl(method, opt), inputBytes = method.I.toBinary(input, opt.binaryOptions), defHeader = new Deferred(), responseStream = new RpcOutputStreamController(), responseEmptyBody = true, maybeStatus, defStatus = new Deferred(), maybeTrailer, defTrailer = new Deferred();
      fetch(url, Object.assign(Object.assign({}, fetchInit), {
        method: "POST",
        headers: createGrpcWebRequestHeader(new globalThis.Headers(), format, opt.timeout, opt.meta),
        body: createGrpcWebRequestBody(inputBytes, format),
        signal: (_d = options.abort) !== null && _d !== void 0 ? _d : null
        // node-fetch@3.0.0-beta.9 rejects `undefined`
      })).then((fetchResponse) => {
        let [code, detail, meta] = readGrpcWebResponseHeader(fetchResponse);
        defHeader.resolve(meta);
        if (code != null && code !== GrpcStatusCode.OK)
          throw new RpcError(detail !== null && detail !== void 0 ? detail : GrpcStatusCode[code], GrpcStatusCode[code], meta);
        if (code != null)
          maybeStatus = {
            code: GrpcStatusCode[code],
            detail: detail !== null && detail !== void 0 ? detail : GrpcStatusCode[code]
          };
        return fetchResponse;
      }).then((fetchResponse) => {
        if (!fetchResponse.body)
          throw new RpcError("missing response body", GrpcStatusCode[GrpcStatusCode.INTERNAL]);
        return readGrpcWebResponseBody(fetchResponse.body, fetchResponse.headers.get("content-type"), (type, data) => {
          switch (type) {
            case GrpcWebFrame.DATA:
              responseStream.notifyMessage(method.O.fromBinary(data, opt.binaryOptions));
              responseEmptyBody = false;
              break;
            case GrpcWebFrame.TRAILER:
              let code, detail;
              [code, detail, maybeTrailer] = readGrpcWebResponseTrailer(data);
              maybeStatus = {
                code: GrpcStatusCode[code],
                detail: detail !== null && detail !== void 0 ? detail : GrpcStatusCode[code]
              };
              break;
          }
        });
      }).then(() => {
        if (!maybeTrailer && !responseEmptyBody)
          throw new RpcError(`missing trailers`, GrpcStatusCode[GrpcStatusCode.DATA_LOSS]);
        if (!maybeStatus)
          throw new RpcError(`missing status`, GrpcStatusCode[GrpcStatusCode.INTERNAL]);
        if (maybeStatus.code !== "OK")
          throw new RpcError(maybeStatus.detail, maybeStatus.code, maybeTrailer);
        responseStream.notifyComplete();
        defStatus.resolve(maybeStatus);
        defTrailer.resolve(maybeTrailer || {});
      }).catch((reason) => {
        let error;
        if (reason instanceof RpcError)
          error = reason;
        else if (reason instanceof Error && reason.name === "AbortError")
          error = new RpcError(reason.message, GrpcStatusCode[GrpcStatusCode.CANCELLED]);
        else
          error = new RpcError(reason instanceof Error ? reason.message : "" + reason, GrpcStatusCode[GrpcStatusCode.INTERNAL]);
        error.methodName = method.name;
        error.serviceName = method.service.typeName;
        defHeader.rejectPending(error);
        responseStream.notifyError(error);
        defStatus.rejectPending(error);
        defTrailer.rejectPending(error);
      });
      return new ServerStreamingCall(method, (_e = opt.meta) !== null && _e !== void 0 ? _e : {}, input, defHeader.promise, responseStream, defStatus.promise, defTrailer.promise);
    }
    unary(method, input, options) {
      var _a, _b, _c, _d, _e;
      let opt = options, format = (_a = opt.format) !== null && _a !== void 0 ? _a : "text", fetch = (_b = opt.fetch) !== null && _b !== void 0 ? _b : globalThis.fetch, fetchInit = (_c = opt.fetchInit) !== null && _c !== void 0 ? _c : {}, url = this.makeUrl(method, opt), inputBytes = method.I.toBinary(input, opt.binaryOptions), defHeader = new Deferred(), maybeMessage, defMessage = new Deferred(), maybeStatus, defStatus = new Deferred(), maybeTrailer, defTrailer = new Deferred();
      fetch(url, Object.assign(Object.assign({}, fetchInit), {
        method: "POST",
        headers: createGrpcWebRequestHeader(new globalThis.Headers(), format, opt.timeout, opt.meta),
        body: createGrpcWebRequestBody(inputBytes, format),
        signal: (_d = options.abort) !== null && _d !== void 0 ? _d : null
        // node-fetch@3.0.0-beta.9 rejects `undefined`
      })).then((fetchResponse) => {
        let [code, detail, meta] = readGrpcWebResponseHeader(fetchResponse);
        defHeader.resolve(meta);
        if (code != null && code !== GrpcStatusCode.OK)
          throw new RpcError(detail !== null && detail !== void 0 ? detail : GrpcStatusCode[code], GrpcStatusCode[code], meta);
        if (code != null)
          maybeStatus = {
            code: GrpcStatusCode[code],
            detail: detail !== null && detail !== void 0 ? detail : GrpcStatusCode[code]
          };
        return fetchResponse;
      }).then((fetchResponse) => {
        if (!fetchResponse.body)
          throw new RpcError("missing response body", GrpcStatusCode[GrpcStatusCode.INTERNAL]);
        return readGrpcWebResponseBody(fetchResponse.body, fetchResponse.headers.get("content-type"), (type, data) => {
          switch (type) {
            case GrpcWebFrame.DATA:
              if (maybeMessage)
                throw new RpcError(`unary call received 2nd message`, GrpcStatusCode[GrpcStatusCode.DATA_LOSS]);
              maybeMessage = method.O.fromBinary(data, opt.binaryOptions);
              break;
            case GrpcWebFrame.TRAILER:
              let code, detail;
              [code, detail, maybeTrailer] = readGrpcWebResponseTrailer(data);
              maybeStatus = {
                code: GrpcStatusCode[code],
                detail: detail !== null && detail !== void 0 ? detail : GrpcStatusCode[code]
              };
              break;
          }
        });
      }).then(() => {
        if (!maybeTrailer && maybeMessage)
          throw new RpcError(`missing trailers`, GrpcStatusCode[GrpcStatusCode.DATA_LOSS]);
        if (!maybeStatus)
          throw new RpcError(`missing status`, GrpcStatusCode[GrpcStatusCode.INTERNAL]);
        if (!maybeMessage && maybeStatus.code === "OK")
          throw new RpcError("expected error status", GrpcStatusCode[GrpcStatusCode.DATA_LOSS]);
        if (!maybeMessage)
          throw new RpcError(maybeStatus.detail, maybeStatus.code, maybeTrailer);
        defMessage.resolve(maybeMessage);
        if (maybeStatus.code !== "OK")
          throw new RpcError(maybeStatus.detail, maybeStatus.code, maybeTrailer);
        defStatus.resolve(maybeStatus);
        defTrailer.resolve(maybeTrailer || {});
      }).catch((reason) => {
        let error;
        if (reason instanceof RpcError)
          error = reason;
        else if (reason instanceof Error && reason.name === "AbortError")
          error = new RpcError(reason.message, GrpcStatusCode[GrpcStatusCode.CANCELLED]);
        else
          error = new RpcError(reason instanceof Error ? reason.message : "" + reason, GrpcStatusCode[GrpcStatusCode.INTERNAL]);
        error.methodName = method.name;
        error.serviceName = method.service.typeName;
        defHeader.rejectPending(error);
        defMessage.rejectPending(error);
        defStatus.rejectPending(error);
        defTrailer.rejectPending(error);
      });
      return new UnaryCall(method, (_e = opt.meta) !== null && _e !== void 0 ? _e : {}, input, defHeader.promise, defMessage.promise, defStatus.promise, defTrailer.promise);
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/json-typings.js
  function typeofJsonValue(value) {
    let t = typeof value;
    if (t == "object") {
      if (Array.isArray(value))
        return "array";
      if (value === null)
        return "null";
    }
    return t;
  }
  function isJsonObject(value) {
    return value !== null && typeof value == "object" && !Array.isArray(value);
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/base64.js
  var encTable2 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".split("");
  var decTable2 = [];
  for (let i = 0; i < encTable2.length; i++)
    decTable2[encTable2[i].charCodeAt(0)] = i;
  decTable2["-".charCodeAt(0)] = encTable2.indexOf("+");
  decTable2["_".charCodeAt(0)] = encTable2.indexOf("/");
  function base64decode2(base64Str) {
    let es = base64Str.length * 3 / 4;
    if (base64Str[base64Str.length - 2] == "=")
      es -= 2;
    else if (base64Str[base64Str.length - 1] == "=")
      es -= 1;
    let bytes = new Uint8Array(es), bytePos = 0, groupPos = 0, b, p = 0;
    for (let i = 0; i < base64Str.length; i++) {
      b = decTable2[base64Str.charCodeAt(i)];
      if (b === void 0) {
        switch (base64Str[i]) {
          case "=":
            groupPos = 0;
          case "\n":
          case "\r":
          case "	":
          case " ":
            continue;
          default:
            throw Error(`invalid base64 string.`);
        }
      }
      switch (groupPos) {
        case 0:
          p = b;
          groupPos = 1;
          break;
        case 1:
          bytes[bytePos++] = p << 2 | (b & 48) >> 4;
          p = b;
          groupPos = 2;
          break;
        case 2:
          bytes[bytePos++] = (p & 15) << 4 | (b & 60) >> 2;
          p = b;
          groupPos = 3;
          break;
        case 3:
          bytes[bytePos++] = (p & 3) << 6 | b;
          groupPos = 0;
          break;
      }
    }
    if (groupPos == 1)
      throw Error(`invalid base64 string.`);
    return bytes.subarray(0, bytePos);
  }
  function base64encode2(bytes) {
    let base64 = "", groupPos = 0, b, p = 0;
    for (let i = 0; i < bytes.length; i++) {
      b = bytes[i];
      switch (groupPos) {
        case 0:
          base64 += encTable2[b >> 2];
          p = (b & 3) << 4;
          groupPos = 1;
          break;
        case 1:
          base64 += encTable2[p | b >> 4];
          p = (b & 15) << 2;
          groupPos = 2;
          break;
        case 2:
          base64 += encTable2[p | b >> 6];
          base64 += encTable2[b & 63];
          groupPos = 0;
          break;
      }
    }
    if (groupPos) {
      base64 += encTable2[p];
      base64 += "=";
      if (groupPos == 1)
        base64 += "=";
    }
    return base64;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/binary-format-contract.js
  var UnknownFieldHandler2;
  (function(UnknownFieldHandler3) {
    UnknownFieldHandler3.symbol = Symbol.for("protobuf-ts/unknown");
    UnknownFieldHandler3.onRead = (typeName, message, fieldNo, wireType, data) => {
      let container = is(message) ? message[UnknownFieldHandler3.symbol] : message[UnknownFieldHandler3.symbol] = [];
      container.push({ no: fieldNo, wireType, data });
    };
    UnknownFieldHandler3.onWrite = (typeName, message, writer) => {
      for (let { no, wireType, data } of UnknownFieldHandler3.list(message))
        writer.tag(no, wireType).raw(data);
    };
    UnknownFieldHandler3.list = (message, fieldNo) => {
      if (is(message)) {
        let all = message[UnknownFieldHandler3.symbol];
        return fieldNo ? all.filter((uf) => uf.no == fieldNo) : all;
      }
      return [];
    };
    UnknownFieldHandler3.last = (message, fieldNo) => UnknownFieldHandler3.list(message, fieldNo).slice(-1)[0];
    const is = (message) => message && Array.isArray(message[UnknownFieldHandler3.symbol]);
  })(UnknownFieldHandler2 || (UnknownFieldHandler2 = {}));
  var WireType2;
  (function(WireType3) {
    WireType3[WireType3["Varint"] = 0] = "Varint";
    WireType3[WireType3["Bit64"] = 1] = "Bit64";
    WireType3[WireType3["LengthDelimited"] = 2] = "LengthDelimited";
    WireType3[WireType3["StartGroup"] = 3] = "StartGroup";
    WireType3[WireType3["EndGroup"] = 4] = "EndGroup";
    WireType3[WireType3["Bit32"] = 5] = "Bit32";
  })(WireType2 || (WireType2 = {}));

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/goog-varint.js
  function varint64read() {
    let lowBits = 0;
    let highBits = 0;
    for (let shift = 0; shift < 28; shift += 7) {
      let b = this.buf[this.pos++];
      lowBits |= (b & 127) << shift;
      if ((b & 128) == 0) {
        this.assertBounds();
        return [lowBits, highBits];
      }
    }
    let middleByte = this.buf[this.pos++];
    lowBits |= (middleByte & 15) << 28;
    highBits = (middleByte & 112) >> 4;
    if ((middleByte & 128) == 0) {
      this.assertBounds();
      return [lowBits, highBits];
    }
    for (let shift = 3; shift <= 31; shift += 7) {
      let b = this.buf[this.pos++];
      highBits |= (b & 127) << shift;
      if ((b & 128) == 0) {
        this.assertBounds();
        return [lowBits, highBits];
      }
    }
    throw new Error("invalid varint");
  }
  function varint64write(lo, hi, bytes) {
    for (let i = 0; i < 28; i = i + 7) {
      const shift = lo >>> i;
      const hasNext = !(shift >>> 7 == 0 && hi == 0);
      const byte = (hasNext ? shift | 128 : shift) & 255;
      bytes.push(byte);
      if (!hasNext) {
        return;
      }
    }
    const splitBits = lo >>> 28 & 15 | (hi & 7) << 4;
    const hasMoreBits = !(hi >> 3 == 0);
    bytes.push((hasMoreBits ? splitBits | 128 : splitBits) & 255);
    if (!hasMoreBits) {
      return;
    }
    for (let i = 3; i < 31; i = i + 7) {
      const shift = hi >>> i;
      const hasNext = !(shift >>> 7 == 0);
      const byte = (hasNext ? shift | 128 : shift) & 255;
      bytes.push(byte);
      if (!hasNext) {
        return;
      }
    }
    bytes.push(hi >>> 31 & 1);
  }
  var TWO_PWR_32_DBL = (1 << 16) * (1 << 16);
  function int64fromString(dec) {
    let minus = dec[0] == "-";
    if (minus)
      dec = dec.slice(1);
    const base = 1e6;
    let lowBits = 0;
    let highBits = 0;
    function add1e6digit(begin, end) {
      const digit1e6 = Number(dec.slice(begin, end));
      highBits *= base;
      lowBits = lowBits * base + digit1e6;
      if (lowBits >= TWO_PWR_32_DBL) {
        highBits = highBits + (lowBits / TWO_PWR_32_DBL | 0);
        lowBits = lowBits % TWO_PWR_32_DBL;
      }
    }
    add1e6digit(-24, -18);
    add1e6digit(-18, -12);
    add1e6digit(-12, -6);
    add1e6digit(-6);
    return [minus, lowBits, highBits];
  }
  function int64toString(bitsLow, bitsHigh) {
    if (bitsHigh >>> 0 <= 2097151) {
      return "" + (TWO_PWR_32_DBL * bitsHigh + (bitsLow >>> 0));
    }
    let low = bitsLow & 16777215;
    let mid = (bitsLow >>> 24 | bitsHigh << 8) >>> 0 & 16777215;
    let high = bitsHigh >> 16 & 65535;
    let digitA = low + mid * 6777216 + high * 6710656;
    let digitB = mid + high * 8147497;
    let digitC = high * 2;
    let base = 1e7;
    if (digitA >= base) {
      digitB += Math.floor(digitA / base);
      digitA %= base;
    }
    if (digitB >= base) {
      digitC += Math.floor(digitB / base);
      digitB %= base;
    }
    function decimalFrom1e7(digit1e7, needLeadingZeros) {
      let partial = digit1e7 ? String(digit1e7) : "";
      if (needLeadingZeros) {
        return "0000000".slice(partial.length) + partial;
      }
      return partial;
    }
    return decimalFrom1e7(
      digitC,
      /*needLeadingZeros=*/
      0
    ) + decimalFrom1e7(
      digitB,
      /*needLeadingZeros=*/
      digitC
    ) + // If the final 1e7 digit didn't need leading zeros, we would have
    // returned via the trivial code path at the top.
    decimalFrom1e7(
      digitA,
      /*needLeadingZeros=*/
      1
    );
  }
  function varint32write(value, bytes) {
    if (value >= 0) {
      while (value > 127) {
        bytes.push(value & 127 | 128);
        value = value >>> 7;
      }
      bytes.push(value);
    } else {
      for (let i = 0; i < 9; i++) {
        bytes.push(value & 127 | 128);
        value = value >> 7;
      }
      bytes.push(1);
    }
  }
  function varint32read() {
    let b = this.buf[this.pos++];
    let result = b & 127;
    if ((b & 128) == 0) {
      this.assertBounds();
      return result;
    }
    b = this.buf[this.pos++];
    result |= (b & 127) << 7;
    if ((b & 128) == 0) {
      this.assertBounds();
      return result;
    }
    b = this.buf[this.pos++];
    result |= (b & 127) << 14;
    if ((b & 128) == 0) {
      this.assertBounds();
      return result;
    }
    b = this.buf[this.pos++];
    result |= (b & 127) << 21;
    if ((b & 128) == 0) {
      this.assertBounds();
      return result;
    }
    b = this.buf[this.pos++];
    result |= (b & 15) << 28;
    for (let readBytes = 5; (b & 128) !== 0 && readBytes < 10; readBytes++)
      b = this.buf[this.pos++];
    if ((b & 128) != 0)
      throw new Error("invalid varint");
    this.assertBounds();
    return result >>> 0;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/pb-long.js
  var BI;
  function detectBi() {
    const dv = new DataView(new ArrayBuffer(8));
    const ok = globalThis.BigInt !== void 0 && typeof dv.getBigInt64 === "function" && typeof dv.getBigUint64 === "function" && typeof dv.setBigInt64 === "function" && typeof dv.setBigUint64 === "function";
    BI = ok ? {
      MIN: BigInt("-9223372036854775808"),
      MAX: BigInt("9223372036854775807"),
      UMIN: BigInt("0"),
      UMAX: BigInt("18446744073709551615"),
      C: BigInt,
      V: dv
    } : void 0;
  }
  detectBi();
  function assertBi(bi) {
    if (!bi)
      throw new Error("BigInt unavailable, see https://github.com/timostamm/protobuf-ts/blob/v1.0.8/MANUAL.md#bigint-support");
  }
  var RE_DECIMAL_STR = /^-?[0-9]+$/;
  var TWO_PWR_32_DBL2 = 4294967296;
  var HALF_2_PWR_32 = 2147483648;
  var SharedPbLong = class {
    /**
     * Create a new instance with the given bits.
     */
    constructor(lo, hi) {
      this.lo = lo | 0;
      this.hi = hi | 0;
    }
    /**
     * Is this instance equal to 0?
     */
    isZero() {
      return this.lo == 0 && this.hi == 0;
    }
    /**
     * Convert to a native number.
     */
    toNumber() {
      let result = this.hi * TWO_PWR_32_DBL2 + (this.lo >>> 0);
      if (!Number.isSafeInteger(result))
        throw new Error("cannot convert to safe number");
      return result;
    }
  };
  var PbULong = class _PbULong extends SharedPbLong {
    /**
     * Create instance from a `string`, `number` or `bigint`.
     */
    static from(value) {
      if (BI)
        switch (typeof value) {
          case "string":
            if (value == "0")
              return this.ZERO;
            if (value == "")
              throw new Error("string is no integer");
            value = BI.C(value);
          case "number":
            if (value === 0)
              return this.ZERO;
            value = BI.C(value);
          case "bigint":
            if (!value)
              return this.ZERO;
            if (value < BI.UMIN)
              throw new Error("signed value for ulong");
            if (value > BI.UMAX)
              throw new Error("ulong too large");
            BI.V.setBigUint64(0, value, true);
            return new _PbULong(BI.V.getInt32(0, true), BI.V.getInt32(4, true));
        }
      else
        switch (typeof value) {
          case "string":
            if (value == "0")
              return this.ZERO;
            value = value.trim();
            if (!RE_DECIMAL_STR.test(value))
              throw new Error("string is no integer");
            let [minus, lo, hi] = int64fromString(value);
            if (minus)
              throw new Error("signed value for ulong");
            return new _PbULong(lo, hi);
          case "number":
            if (value == 0)
              return this.ZERO;
            if (!Number.isSafeInteger(value))
              throw new Error("number is no integer");
            if (value < 0)
              throw new Error("signed value for ulong");
            return new _PbULong(value, value / TWO_PWR_32_DBL2);
        }
      throw new Error("unknown value " + typeof value);
    }
    /**
     * Convert to decimal string.
     */
    toString() {
      return BI ? this.toBigInt().toString() : int64toString(this.lo, this.hi);
    }
    /**
     * Convert to native bigint.
     */
    toBigInt() {
      assertBi(BI);
      BI.V.setInt32(0, this.lo, true);
      BI.V.setInt32(4, this.hi, true);
      return BI.V.getBigUint64(0, true);
    }
  };
  PbULong.ZERO = new PbULong(0, 0);
  var PbLong = class _PbLong extends SharedPbLong {
    /**
     * Create instance from a `string`, `number` or `bigint`.
     */
    static from(value) {
      if (BI)
        switch (typeof value) {
          case "string":
            if (value == "0")
              return this.ZERO;
            if (value == "")
              throw new Error("string is no integer");
            value = BI.C(value);
          case "number":
            if (value === 0)
              return this.ZERO;
            value = BI.C(value);
          case "bigint":
            if (!value)
              return this.ZERO;
            if (value < BI.MIN)
              throw new Error("signed long too small");
            if (value > BI.MAX)
              throw new Error("signed long too large");
            BI.V.setBigInt64(0, value, true);
            return new _PbLong(BI.V.getInt32(0, true), BI.V.getInt32(4, true));
        }
      else
        switch (typeof value) {
          case "string":
            if (value == "0")
              return this.ZERO;
            value = value.trim();
            if (!RE_DECIMAL_STR.test(value))
              throw new Error("string is no integer");
            let [minus, lo, hi] = int64fromString(value);
            if (minus) {
              if (hi > HALF_2_PWR_32 || hi == HALF_2_PWR_32 && lo != 0)
                throw new Error("signed long too small");
            } else if (hi >= HALF_2_PWR_32)
              throw new Error("signed long too large");
            let pbl = new _PbLong(lo, hi);
            return minus ? pbl.negate() : pbl;
          case "number":
            if (value == 0)
              return this.ZERO;
            if (!Number.isSafeInteger(value))
              throw new Error("number is no integer");
            return value > 0 ? new _PbLong(value, value / TWO_PWR_32_DBL2) : new _PbLong(-value, -value / TWO_PWR_32_DBL2).negate();
        }
      throw new Error("unknown value " + typeof value);
    }
    /**
     * Do we have a minus sign?
     */
    isNegative() {
      return (this.hi & HALF_2_PWR_32) !== 0;
    }
    /**
     * Negate two's complement.
     * Invert all the bits and add one to the result.
     */
    negate() {
      let hi = ~this.hi, lo = this.lo;
      if (lo)
        lo = ~lo + 1;
      else
        hi += 1;
      return new _PbLong(lo, hi);
    }
    /**
     * Convert to decimal string.
     */
    toString() {
      if (BI)
        return this.toBigInt().toString();
      if (this.isNegative()) {
        let n = this.negate();
        return "-" + int64toString(n.lo, n.hi);
      }
      return int64toString(this.lo, this.hi);
    }
    /**
     * Convert to native bigint.
     */
    toBigInt() {
      assertBi(BI);
      BI.V.setInt32(0, this.lo, true);
      BI.V.setInt32(4, this.hi, true);
      return BI.V.getBigInt64(0, true);
    }
  };
  PbLong.ZERO = new PbLong(0, 0);

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/binary-reader.js
  var defaultsRead = {
    readUnknownField: true,
    readerFactory: (bytes) => new BinaryReader(bytes)
  };
  function binaryReadOptions(options) {
    return options ? Object.assign(Object.assign({}, defaultsRead), options) : defaultsRead;
  }
  var BinaryReader = class {
    constructor(buf, textDecoder) {
      this.varint64 = varint64read;
      this.uint32 = varint32read;
      this.buf = buf;
      this.len = buf.length;
      this.pos = 0;
      this.view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
      this.textDecoder = textDecoder !== null && textDecoder !== void 0 ? textDecoder : new TextDecoder("utf-8", {
        fatal: true,
        ignoreBOM: true
      });
    }
    /**
     * Reads a tag - field number and wire type.
     */
    tag() {
      let tag = this.uint32(), fieldNo = tag >>> 3, wireType = tag & 7;
      if (fieldNo <= 0 || wireType < 0 || wireType > 5)
        throw new Error("illegal tag: field no " + fieldNo + " wire type " + wireType);
      return [fieldNo, wireType];
    }
    /**
     * Skip one element on the wire and return the skipped data.
     * Supports WireType.StartGroup since v2.0.0-alpha.23.
     */
    skip(wireType) {
      let start = this.pos;
      switch (wireType) {
        case WireType2.Varint:
          while (this.buf[this.pos++] & 128) {
          }
          break;
        case WireType2.Bit64:
          this.pos += 4;
        case WireType2.Bit32:
          this.pos += 4;
          break;
        case WireType2.LengthDelimited:
          let len = this.uint32();
          this.pos += len;
          break;
        case WireType2.StartGroup:
          let t;
          while ((t = this.tag()[1]) !== WireType2.EndGroup) {
            this.skip(t);
          }
          break;
        default:
          throw new Error("cant skip wire type " + wireType);
      }
      this.assertBounds();
      return this.buf.subarray(start, this.pos);
    }
    /**
     * Throws error if position in byte array is out of range.
     */
    assertBounds() {
      if (this.pos > this.len)
        throw new RangeError("premature EOF");
    }
    /**
     * Read a `int32` field, a signed 32 bit varint.
     */
    int32() {
      return this.uint32() | 0;
    }
    /**
     * Read a `sint32` field, a signed, zigzag-encoded 32-bit varint.
     */
    sint32() {
      let zze = this.uint32();
      return zze >>> 1 ^ -(zze & 1);
    }
    /**
     * Read a `int64` field, a signed 64-bit varint.
     */
    int64() {
      return new PbLong(...this.varint64());
    }
    /**
     * Read a `uint64` field, an unsigned 64-bit varint.
     */
    uint64() {
      return new PbULong(...this.varint64());
    }
    /**
     * Read a `sint64` field, a signed, zig-zag-encoded 64-bit varint.
     */
    sint64() {
      let [lo, hi] = this.varint64();
      let s = -(lo & 1);
      lo = (lo >>> 1 | (hi & 1) << 31) ^ s;
      hi = hi >>> 1 ^ s;
      return new PbLong(lo, hi);
    }
    /**
     * Read a `bool` field, a variant.
     */
    bool() {
      let [lo, hi] = this.varint64();
      return lo !== 0 || hi !== 0;
    }
    /**
     * Read a `fixed32` field, an unsigned, fixed-length 32-bit integer.
     */
    fixed32() {
      return this.view.getUint32((this.pos += 4) - 4, true);
    }
    /**
     * Read a `sfixed32` field, a signed, fixed-length 32-bit integer.
     */
    sfixed32() {
      return this.view.getInt32((this.pos += 4) - 4, true);
    }
    /**
     * Read a `fixed64` field, an unsigned, fixed-length 64 bit integer.
     */
    fixed64() {
      return new PbULong(this.sfixed32(), this.sfixed32());
    }
    /**
     * Read a `fixed64` field, a signed, fixed-length 64-bit integer.
     */
    sfixed64() {
      return new PbLong(this.sfixed32(), this.sfixed32());
    }
    /**
     * Read a `float` field, 32-bit floating point number.
     */
    float() {
      return this.view.getFloat32((this.pos += 4) - 4, true);
    }
    /**
     * Read a `double` field, a 64-bit floating point number.
     */
    double() {
      return this.view.getFloat64((this.pos += 8) - 8, true);
    }
    /**
     * Read a `bytes` field, length-delimited arbitrary data.
     */
    bytes() {
      let len = this.uint32();
      let start = this.pos;
      this.pos += len;
      this.assertBounds();
      return this.buf.subarray(start, start + len);
    }
    /**
     * Read a `string` field, length-delimited data converted to UTF-8 text.
     */
    string() {
      return this.textDecoder.decode(this.bytes());
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/assert.js
  function assert2(condition, msg) {
    if (!condition) {
      throw new Error(msg);
    }
  }
  function assertNever2(value, msg) {
    throw new Error(msg !== null && msg !== void 0 ? msg : "Unexpected object: " + value);
  }
  var FLOAT32_MAX = 34028234663852886e22;
  var FLOAT32_MIN = -34028234663852886e22;
  var UINT32_MAX = 4294967295;
  var INT32_MAX = 2147483647;
  var INT32_MIN = -2147483648;
  function assertInt322(arg) {
    if (typeof arg !== "number")
      throw new Error("invalid int 32: " + typeof arg);
    if (!Number.isInteger(arg) || arg > INT32_MAX || arg < INT32_MIN)
      throw new Error("invalid int 32: " + arg);
  }
  function assertUInt322(arg) {
    if (typeof arg !== "number")
      throw new Error("invalid uint 32: " + typeof arg);
    if (!Number.isInteger(arg) || arg > UINT32_MAX || arg < 0)
      throw new Error("invalid uint 32: " + arg);
  }
  function assertFloat322(arg) {
    if (typeof arg !== "number")
      throw new Error("invalid float 32: " + typeof arg);
    if (!Number.isFinite(arg))
      return;
    if (arg > FLOAT32_MAX || arg < FLOAT32_MIN)
      throw new Error("invalid float 32: " + arg);
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/binary-writer.js
  var defaultsWrite = {
    writeUnknownFields: true,
    writerFactory: () => new BinaryWriter()
  };
  function binaryWriteOptions(options) {
    return options ? Object.assign(Object.assign({}, defaultsWrite), options) : defaultsWrite;
  }
  var BinaryWriter = class {
    constructor(textEncoder) {
      this.stack = [];
      this.textEncoder = textEncoder !== null && textEncoder !== void 0 ? textEncoder : new TextEncoder();
      this.chunks = [];
      this.buf = [];
    }
    /**
     * Return all bytes written and reset this writer.
     */
    finish() {
      this.chunks.push(new Uint8Array(this.buf));
      let len = 0;
      for (let i = 0; i < this.chunks.length; i++)
        len += this.chunks[i].length;
      let bytes = new Uint8Array(len);
      let offset = 0;
      for (let i = 0; i < this.chunks.length; i++) {
        bytes.set(this.chunks[i], offset);
        offset += this.chunks[i].length;
      }
      this.chunks = [];
      return bytes;
    }
    /**
     * Start a new fork for length-delimited data like a message
     * or a packed repeated field.
     *
     * Must be joined later with `join()`.
     */
    fork() {
      this.stack.push({ chunks: this.chunks, buf: this.buf });
      this.chunks = [];
      this.buf = [];
      return this;
    }
    /**
     * Join the last fork. Write its length and bytes, then
     * return to the previous state.
     */
    join() {
      let chunk = this.finish();
      let prev = this.stack.pop();
      if (!prev)
        throw new Error("invalid state, fork stack empty");
      this.chunks = prev.chunks;
      this.buf = prev.buf;
      this.uint32(chunk.byteLength);
      return this.raw(chunk);
    }
    /**
     * Writes a tag (field number and wire type).
     *
     * Equivalent to `uint32( (fieldNo << 3 | type) >>> 0 )`.
     *
     * Generated code should compute the tag ahead of time and call `uint32()`.
     */
    tag(fieldNo, type) {
      return this.uint32((fieldNo << 3 | type) >>> 0);
    }
    /**
     * Write a chunk of raw bytes.
     */
    raw(chunk) {
      if (this.buf.length) {
        this.chunks.push(new Uint8Array(this.buf));
        this.buf = [];
      }
      this.chunks.push(chunk);
      return this;
    }
    /**
     * Write a `uint32` value, an unsigned 32 bit varint.
     */
    uint32(value) {
      assertUInt322(value);
      while (value > 127) {
        this.buf.push(value & 127 | 128);
        value = value >>> 7;
      }
      this.buf.push(value);
      return this;
    }
    /**
     * Write a `int32` value, a signed 32 bit varint.
     */
    int32(value) {
      assertInt322(value);
      varint32write(value, this.buf);
      return this;
    }
    /**
     * Write a `bool` value, a variant.
     */
    bool(value) {
      this.buf.push(value ? 1 : 0);
      return this;
    }
    /**
     * Write a `bytes` value, length-delimited arbitrary data.
     */
    bytes(value) {
      this.uint32(value.byteLength);
      return this.raw(value);
    }
    /**
     * Write a `string` value, length-delimited data converted to UTF-8 text.
     */
    string(value) {
      let chunk = this.textEncoder.encode(value);
      this.uint32(chunk.byteLength);
      return this.raw(chunk);
    }
    /**
     * Write a `float` value, 32-bit floating point number.
     */
    float(value) {
      assertFloat322(value);
      let chunk = new Uint8Array(4);
      new DataView(chunk.buffer).setFloat32(0, value, true);
      return this.raw(chunk);
    }
    /**
     * Write a `double` value, a 64-bit floating point number.
     */
    double(value) {
      let chunk = new Uint8Array(8);
      new DataView(chunk.buffer).setFloat64(0, value, true);
      return this.raw(chunk);
    }
    /**
     * Write a `fixed32` value, an unsigned, fixed-length 32-bit integer.
     */
    fixed32(value) {
      assertUInt322(value);
      let chunk = new Uint8Array(4);
      new DataView(chunk.buffer).setUint32(0, value, true);
      return this.raw(chunk);
    }
    /**
     * Write a `sfixed32` value, a signed, fixed-length 32-bit integer.
     */
    sfixed32(value) {
      assertInt322(value);
      let chunk = new Uint8Array(4);
      new DataView(chunk.buffer).setInt32(0, value, true);
      return this.raw(chunk);
    }
    /**
     * Write a `sint32` value, a signed, zigzag-encoded 32-bit varint.
     */
    sint32(value) {
      assertInt322(value);
      value = (value << 1 ^ value >> 31) >>> 0;
      varint32write(value, this.buf);
      return this;
    }
    /**
     * Write a `fixed64` value, a signed, fixed-length 64-bit integer.
     */
    sfixed64(value) {
      let chunk = new Uint8Array(8);
      let view = new DataView(chunk.buffer);
      let long = PbLong.from(value);
      view.setInt32(0, long.lo, true);
      view.setInt32(4, long.hi, true);
      return this.raw(chunk);
    }
    /**
     * Write a `fixed64` value, an unsigned, fixed-length 64 bit integer.
     */
    fixed64(value) {
      let chunk = new Uint8Array(8);
      let view = new DataView(chunk.buffer);
      let long = PbULong.from(value);
      view.setInt32(0, long.lo, true);
      view.setInt32(4, long.hi, true);
      return this.raw(chunk);
    }
    /**
     * Write a `int64` value, a signed 64-bit varint.
     */
    int64(value) {
      let long = PbLong.from(value);
      varint64write(long.lo, long.hi, this.buf);
      return this;
    }
    /**
     * Write a `sint64` value, a signed, zig-zag-encoded 64-bit varint.
     */
    sint64(value) {
      let long = PbLong.from(value), sign = long.hi >> 31, lo = long.lo << 1 ^ sign, hi = (long.hi << 1 | long.lo >>> 31) ^ sign;
      varint64write(lo, hi, this.buf);
      return this;
    }
    /**
     * Write a `uint64` value, an unsigned 64-bit varint.
     */
    uint64(value) {
      let long = PbULong.from(value);
      varint64write(long.lo, long.hi, this.buf);
      return this;
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/json-format-contract.js
  var defaultsWrite2 = {
    emitDefaultValues: false,
    enumAsInteger: false,
    useProtoFieldName: false,
    prettySpaces: 0
  };
  var defaultsRead2 = {
    ignoreUnknownFields: false
  };
  function jsonReadOptions2(options) {
    return options ? Object.assign(Object.assign({}, defaultsRead2), options) : defaultsRead2;
  }
  function jsonWriteOptions2(options) {
    return options ? Object.assign(Object.assign({}, defaultsWrite2), options) : defaultsWrite2;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/message-type-contract.js
  var MESSAGE_TYPE = Symbol.for("protobuf-ts/message-type");

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/lower-camel-case.js
  function lowerCamelCase(snakeCase) {
    let capNext = false;
    const sb = [];
    for (let i = 0; i < snakeCase.length; i++) {
      let next = snakeCase.charAt(i);
      if (next == "_") {
        capNext = true;
      } else if (/\d/.test(next)) {
        sb.push(next);
        capNext = true;
      } else if (capNext) {
        sb.push(next.toUpperCase());
        capNext = false;
      } else if (i == 0) {
        sb.push(next.toLowerCase());
      } else {
        sb.push(next);
      }
    }
    return sb.join("");
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-info.js
  var ScalarType;
  (function(ScalarType2) {
    ScalarType2[ScalarType2["DOUBLE"] = 1] = "DOUBLE";
    ScalarType2[ScalarType2["FLOAT"] = 2] = "FLOAT";
    ScalarType2[ScalarType2["INT64"] = 3] = "INT64";
    ScalarType2[ScalarType2["UINT64"] = 4] = "UINT64";
    ScalarType2[ScalarType2["INT32"] = 5] = "INT32";
    ScalarType2[ScalarType2["FIXED64"] = 6] = "FIXED64";
    ScalarType2[ScalarType2["FIXED32"] = 7] = "FIXED32";
    ScalarType2[ScalarType2["BOOL"] = 8] = "BOOL";
    ScalarType2[ScalarType2["STRING"] = 9] = "STRING";
    ScalarType2[ScalarType2["BYTES"] = 12] = "BYTES";
    ScalarType2[ScalarType2["UINT32"] = 13] = "UINT32";
    ScalarType2[ScalarType2["SFIXED32"] = 15] = "SFIXED32";
    ScalarType2[ScalarType2["SFIXED64"] = 16] = "SFIXED64";
    ScalarType2[ScalarType2["SINT32"] = 17] = "SINT32";
    ScalarType2[ScalarType2["SINT64"] = 18] = "SINT64";
  })(ScalarType || (ScalarType = {}));
  var LongType;
  (function(LongType2) {
    LongType2[LongType2["BIGINT"] = 0] = "BIGINT";
    LongType2[LongType2["STRING"] = 1] = "STRING";
    LongType2[LongType2["NUMBER"] = 2] = "NUMBER";
  })(LongType || (LongType = {}));
  var RepeatType;
  (function(RepeatType2) {
    RepeatType2[RepeatType2["NO"] = 0] = "NO";
    RepeatType2[RepeatType2["PACKED"] = 1] = "PACKED";
    RepeatType2[RepeatType2["UNPACKED"] = 2] = "UNPACKED";
  })(RepeatType || (RepeatType = {}));
  function normalizeFieldInfo(field) {
    var _a, _b, _c, _d;
    field.localName = (_a = field.localName) !== null && _a !== void 0 ? _a : lowerCamelCase(field.name);
    field.jsonName = (_b = field.jsonName) !== null && _b !== void 0 ? _b : lowerCamelCase(field.name);
    field.repeat = (_c = field.repeat) !== null && _c !== void 0 ? _c : RepeatType.NO;
    field.opt = (_d = field.opt) !== null && _d !== void 0 ? _d : field.repeat ? false : field.oneof ? false : field.kind == "message";
    return field;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/oneof.js
  function isOneofGroup(any) {
    if (typeof any != "object" || any === null || !any.hasOwnProperty("oneofKind")) {
      return false;
    }
    switch (typeof any.oneofKind) {
      case "string":
        if (any[any.oneofKind] === void 0)
          return false;
        return Object.keys(any).length == 2;
      case "undefined":
        return Object.keys(any).length == 1;
      default:
        return false;
    }
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-type-check.js
  var ReflectionTypeCheck = class {
    constructor(info) {
      var _a;
      this.fields = (_a = info.fields) !== null && _a !== void 0 ? _a : [];
    }
    prepare() {
      if (this.data)
        return;
      const req = [], known = [], oneofs = [];
      for (let field of this.fields) {
        if (field.oneof) {
          if (!oneofs.includes(field.oneof)) {
            oneofs.push(field.oneof);
            req.push(field.oneof);
            known.push(field.oneof);
          }
        } else {
          known.push(field.localName);
          switch (field.kind) {
            case "scalar":
            case "enum":
              if (!field.opt || field.repeat)
                req.push(field.localName);
              break;
            case "message":
              if (field.repeat)
                req.push(field.localName);
              break;
            case "map":
              req.push(field.localName);
              break;
          }
        }
      }
      this.data = { req, known, oneofs: Object.values(oneofs) };
    }
    /**
     * Is the argument a valid message as specified by the
     * reflection information?
     *
     * Checks all field types recursively. The `depth`
     * specifies how deep into the structure the check will be.
     *
     * With a depth of 0, only the presence of fields
     * is checked.
     *
     * With a depth of 1 or more, the field types are checked.
     *
     * With a depth of 2 or more, the members of map, repeated
     * and message fields are checked.
     *
     * Message fields will be checked recursively with depth - 1.
     *
     * The number of map entries / repeated values being checked
     * is < depth.
     */
    is(message, depth, allowExcessProperties = false) {
      if (depth < 0)
        return true;
      if (message === null || message === void 0 || typeof message != "object")
        return false;
      this.prepare();
      let keys = Object.keys(message), data = this.data;
      if (keys.length < data.req.length || data.req.some((n) => !keys.includes(n)))
        return false;
      if (!allowExcessProperties) {
        if (keys.some((k) => !data.known.includes(k)))
          return false;
      }
      if (depth < 1) {
        return true;
      }
      for (const name of data.oneofs) {
        const group = message[name];
        if (!isOneofGroup(group))
          return false;
        if (group.oneofKind === void 0)
          continue;
        const field = this.fields.find((f) => f.localName === group.oneofKind);
        if (!field)
          return false;
        if (!this.field(group[group.oneofKind], field, allowExcessProperties, depth))
          return false;
      }
      for (const field of this.fields) {
        if (field.oneof !== void 0)
          continue;
        if (!this.field(message[field.localName], field, allowExcessProperties, depth))
          return false;
      }
      return true;
    }
    field(arg, field, allowExcessProperties, depth) {
      let repeated = field.repeat;
      switch (field.kind) {
        case "scalar":
          if (arg === void 0)
            return field.opt;
          if (repeated)
            return this.scalars(arg, field.T, depth, field.L);
          return this.scalar(arg, field.T, field.L);
        case "enum":
          if (arg === void 0)
            return field.opt;
          if (repeated)
            return this.scalars(arg, ScalarType.INT32, depth);
          return this.scalar(arg, ScalarType.INT32);
        case "message":
          if (arg === void 0)
            return true;
          if (repeated)
            return this.messages(arg, field.T(), allowExcessProperties, depth);
          return this.message(arg, field.T(), allowExcessProperties, depth);
        case "map":
          if (typeof arg != "object" || arg === null)
            return false;
          if (depth < 2)
            return true;
          if (!this.mapKeys(arg, field.K, depth))
            return false;
          switch (field.V.kind) {
            case "scalar":
              return this.scalars(Object.values(arg), field.V.T, depth, field.V.L);
            case "enum":
              return this.scalars(Object.values(arg), ScalarType.INT32, depth);
            case "message":
              return this.messages(Object.values(arg), field.V.T(), allowExcessProperties, depth);
          }
          break;
      }
      return true;
    }
    message(arg, type, allowExcessProperties, depth) {
      if (allowExcessProperties) {
        return type.isAssignable(arg, depth);
      }
      return type.is(arg, depth);
    }
    messages(arg, type, allowExcessProperties, depth) {
      if (!Array.isArray(arg))
        return false;
      if (depth < 2)
        return true;
      if (allowExcessProperties) {
        for (let i = 0; i < arg.length && i < depth; i++)
          if (!type.isAssignable(arg[i], depth - 1))
            return false;
      } else {
        for (let i = 0; i < arg.length && i < depth; i++)
          if (!type.is(arg[i], depth - 1))
            return false;
      }
      return true;
    }
    scalar(arg, type, longType) {
      let argType = typeof arg;
      switch (type) {
        case ScalarType.UINT64:
        case ScalarType.FIXED64:
        case ScalarType.INT64:
        case ScalarType.SFIXED64:
        case ScalarType.SINT64:
          switch (longType) {
            case LongType.BIGINT:
              return argType == "bigint";
            case LongType.NUMBER:
              return argType == "number" && !isNaN(arg);
            default:
              return argType == "string";
          }
        case ScalarType.BOOL:
          return argType == "boolean";
        case ScalarType.STRING:
          return argType == "string";
        case ScalarType.BYTES:
          return arg instanceof Uint8Array;
        case ScalarType.DOUBLE:
        case ScalarType.FLOAT:
          return argType == "number" && !isNaN(arg);
        default:
          return argType == "number" && Number.isInteger(arg);
      }
    }
    scalars(arg, type, depth, longType) {
      if (!Array.isArray(arg))
        return false;
      if (depth < 2)
        return true;
      if (Array.isArray(arg)) {
        for (let i = 0; i < arg.length && i < depth; i++)
          if (!this.scalar(arg[i], type, longType))
            return false;
      }
      return true;
    }
    mapKeys(map, type, depth) {
      let keys = Object.keys(map);
      switch (type) {
        case ScalarType.INT32:
        case ScalarType.FIXED32:
        case ScalarType.SFIXED32:
        case ScalarType.SINT32:
        case ScalarType.UINT32:
          return this.scalars(keys.slice(0, depth).map((k) => parseInt(k)), type, depth);
        case ScalarType.BOOL:
          return this.scalars(keys.slice(0, depth).map((k) => k == "true" ? true : k == "false" ? false : k), type, depth);
        default:
          return this.scalars(keys, type, depth, LongType.STRING);
      }
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-long-convert.js
  function reflectionLongConvert(long, type) {
    switch (type) {
      case LongType.BIGINT:
        return long.toBigInt();
      case LongType.NUMBER:
        return long.toNumber();
      default:
        return long.toString();
    }
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-json-reader.js
  var ReflectionJsonReader = class {
    constructor(info) {
      this.info = info;
    }
    prepare() {
      var _a;
      if (this.fMap === void 0) {
        this.fMap = {};
        const fieldsInput = (_a = this.info.fields) !== null && _a !== void 0 ? _a : [];
        for (const field of fieldsInput) {
          this.fMap[field.name] = field;
          this.fMap[field.jsonName] = field;
          this.fMap[field.localName] = field;
        }
      }
    }
    // Cannot parse JSON <type of jsonValue> for <type name>#<fieldName>.
    assert(condition, fieldName, jsonValue) {
      if (!condition) {
        let what = typeofJsonValue(jsonValue);
        if (what == "number" || what == "boolean")
          what = jsonValue.toString();
        throw new Error(`Cannot parse JSON ${what} for ${this.info.typeName}#${fieldName}`);
      }
    }
    /**
     * Reads a message from canonical JSON format into the target message.
     *
     * Repeated fields are appended. Map entries are added, overwriting
     * existing keys.
     *
     * If a message field is already present, it will be merged with the
     * new data.
     */
    read(input, message, options) {
      this.prepare();
      const oneofsHandled = [];
      for (const [jsonKey, jsonValue] of Object.entries(input)) {
        const field = this.fMap[jsonKey];
        if (!field) {
          if (!options.ignoreUnknownFields)
            throw new Error(`Found unknown field while reading ${this.info.typeName} from JSON format. JSON key: ${jsonKey}`);
          continue;
        }
        const localName = field.localName;
        let target;
        if (field.oneof) {
          if (jsonValue === null && (field.kind !== "enum" || field.T()[0] !== "google.protobuf.NullValue")) {
            continue;
          }
          if (oneofsHandled.includes(field.oneof))
            throw new Error(`Multiple members of the oneof group "${field.oneof}" of ${this.info.typeName} are present in JSON.`);
          oneofsHandled.push(field.oneof);
          target = message[field.oneof] = {
            oneofKind: localName
          };
        } else {
          target = message;
        }
        if (field.kind == "map") {
          if (jsonValue === null) {
            continue;
          }
          this.assert(isJsonObject(jsonValue), field.name, jsonValue);
          const fieldObj = target[localName];
          for (const [jsonObjKey, jsonObjValue] of Object.entries(jsonValue)) {
            this.assert(jsonObjValue !== null, field.name + " map value", null);
            let val;
            switch (field.V.kind) {
              case "message":
                val = field.V.T().internalJsonRead(jsonObjValue, options);
                break;
              case "enum":
                val = this.enum(field.V.T(), jsonObjValue, field.name, options.ignoreUnknownFields);
                if (val === false)
                  continue;
                break;
              case "scalar":
                val = this.scalar(jsonObjValue, field.V.T, field.V.L, field.name);
                break;
            }
            this.assert(val !== void 0, field.name + " map value", jsonObjValue);
            let key = jsonObjKey;
            if (field.K == ScalarType.BOOL)
              key = key == "true" ? true : key == "false" ? false : key;
            key = this.scalar(key, field.K, LongType.STRING, field.name).toString();
            fieldObj[key] = val;
          }
        } else if (field.repeat) {
          if (jsonValue === null)
            continue;
          this.assert(Array.isArray(jsonValue), field.name, jsonValue);
          const fieldArr = target[localName];
          for (const jsonItem of jsonValue) {
            this.assert(jsonItem !== null, field.name, null);
            let val;
            switch (field.kind) {
              case "message":
                val = field.T().internalJsonRead(jsonItem, options);
                break;
              case "enum":
                val = this.enum(field.T(), jsonItem, field.name, options.ignoreUnknownFields);
                if (val === false)
                  continue;
                break;
              case "scalar":
                val = this.scalar(jsonItem, field.T, field.L, field.name);
                break;
            }
            this.assert(val !== void 0, field.name, jsonValue);
            fieldArr.push(val);
          }
        } else {
          switch (field.kind) {
            case "message":
              if (jsonValue === null && field.T().typeName != "google.protobuf.Value") {
                this.assert(field.oneof === void 0, field.name + " (oneof member)", null);
                continue;
              }
              target[localName] = field.T().internalJsonRead(jsonValue, options, target[localName]);
              break;
            case "enum":
              if (jsonValue === null)
                continue;
              let val = this.enum(field.T(), jsonValue, field.name, options.ignoreUnknownFields);
              if (val === false)
                continue;
              target[localName] = val;
              break;
            case "scalar":
              if (jsonValue === null)
                continue;
              target[localName] = this.scalar(jsonValue, field.T, field.L, field.name);
              break;
          }
        }
      }
    }
    /**
     * Returns `false` for unrecognized string representations.
     *
     * google.protobuf.NullValue accepts only JSON `null` (or the old `"NULL_VALUE"`).
     */
    enum(type, json, fieldName, ignoreUnknownFields) {
      if (type[0] == "google.protobuf.NullValue")
        assert2(json === null || json === "NULL_VALUE", `Unable to parse field ${this.info.typeName}#${fieldName}, enum ${type[0]} only accepts null.`);
      if (json === null)
        return 0;
      switch (typeof json) {
        case "number":
          assert2(Number.isInteger(json), `Unable to parse field ${this.info.typeName}#${fieldName}, enum can only be integral number, got ${json}.`);
          return json;
        case "string":
          let localEnumName = json;
          if (type[2] && json.substring(0, type[2].length) === type[2])
            localEnumName = json.substring(type[2].length);
          let enumNumber = type[1][localEnumName];
          if (typeof enumNumber === "undefined" && ignoreUnknownFields) {
            return false;
          }
          assert2(typeof enumNumber == "number", `Unable to parse field ${this.info.typeName}#${fieldName}, enum ${type[0]} has no value for "${json}".`);
          return enumNumber;
      }
      assert2(false, `Unable to parse field ${this.info.typeName}#${fieldName}, cannot parse enum value from ${typeof json}".`);
    }
    scalar(json, type, longType, fieldName) {
      let e;
      try {
        switch (type) {
          case ScalarType.DOUBLE:
          case ScalarType.FLOAT:
            if (json === null)
              return 0;
            if (json === "NaN")
              return Number.NaN;
            if (json === "Infinity")
              return Number.POSITIVE_INFINITY;
            if (json === "-Infinity")
              return Number.NEGATIVE_INFINITY;
            if (json === "") {
              e = "empty string";
              break;
            }
            if (typeof json == "string" && json.trim().length !== json.length) {
              e = "extra whitespace";
              break;
            }
            if (typeof json != "string" && typeof json != "number") {
              break;
            }
            let float = Number(json);
            if (Number.isNaN(float)) {
              e = "not a number";
              break;
            }
            if (!Number.isFinite(float)) {
              e = "too large or small";
              break;
            }
            if (type == ScalarType.FLOAT)
              assertFloat322(float);
            return float;
          case ScalarType.INT32:
          case ScalarType.FIXED32:
          case ScalarType.SFIXED32:
          case ScalarType.SINT32:
          case ScalarType.UINT32:
            if (json === null)
              return 0;
            let int32;
            if (typeof json == "number")
              int32 = json;
            else if (json === "")
              e = "empty string";
            else if (typeof json == "string") {
              if (json.trim().length !== json.length)
                e = "extra whitespace";
              else
                int32 = Number(json);
            }
            if (int32 === void 0)
              break;
            if (type == ScalarType.UINT32)
              assertUInt322(int32);
            else
              assertInt322(int32);
            return int32;
          case ScalarType.INT64:
          case ScalarType.SFIXED64:
          case ScalarType.SINT64:
            if (json === null)
              return reflectionLongConvert(PbLong.ZERO, longType);
            if (typeof json != "number" && typeof json != "string")
              break;
            return reflectionLongConvert(PbLong.from(json), longType);
          case ScalarType.FIXED64:
          case ScalarType.UINT64:
            if (json === null)
              return reflectionLongConvert(PbULong.ZERO, longType);
            if (typeof json != "number" && typeof json != "string")
              break;
            return reflectionLongConvert(PbULong.from(json), longType);
          case ScalarType.BOOL:
            if (json === null)
              return false;
            if (typeof json !== "boolean")
              break;
            return json;
          case ScalarType.STRING:
            if (json === null)
              return "";
            if (typeof json !== "string") {
              e = "extra whitespace";
              break;
            }
            try {
              encodeURIComponent(json);
            } catch (e2) {
              e2 = "invalid UTF8";
              break;
            }
            return json;
          case ScalarType.BYTES:
            if (json === null || json === "")
              return new Uint8Array(0);
            if (typeof json !== "string")
              break;
            return base64decode2(json);
        }
      } catch (error) {
        e = error.message;
      }
      this.assert(false, fieldName + (e ? " - " + e : ""), json);
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-json-writer.js
  var ReflectionJsonWriter = class {
    constructor(info) {
      var _a;
      this.fields = (_a = info.fields) !== null && _a !== void 0 ? _a : [];
    }
    /**
     * Converts the message to a JSON object, based on the field descriptors.
     */
    write(message, options) {
      const json = {}, source = message;
      for (const field of this.fields) {
        if (!field.oneof) {
          let jsonValue2 = this.field(field, source[field.localName], options);
          if (jsonValue2 !== void 0)
            json[options.useProtoFieldName ? field.name : field.jsonName] = jsonValue2;
          continue;
        }
        const group = source[field.oneof];
        if (group.oneofKind !== field.localName)
          continue;
        const opt = field.kind == "scalar" || field.kind == "enum" ? Object.assign(Object.assign({}, options), { emitDefaultValues: true }) : options;
        let jsonValue = this.field(field, group[field.localName], opt);
        assert2(jsonValue !== void 0);
        json[options.useProtoFieldName ? field.name : field.jsonName] = jsonValue;
      }
      return json;
    }
    field(field, value, options) {
      let jsonValue = void 0;
      if (field.kind == "map") {
        assert2(typeof value == "object" && value !== null);
        const jsonObj = {};
        switch (field.V.kind) {
          case "scalar":
            for (const [entryKey, entryValue] of Object.entries(value)) {
              const val = this.scalar(field.V.T, entryValue, field.name, false, true);
              assert2(val !== void 0);
              jsonObj[entryKey.toString()] = val;
            }
            break;
          case "message":
            const messageType = field.V.T();
            for (const [entryKey, entryValue] of Object.entries(value)) {
              const val = this.message(messageType, entryValue, field.name, options);
              assert2(val !== void 0);
              jsonObj[entryKey.toString()] = val;
            }
            break;
          case "enum":
            const enumInfo = field.V.T();
            for (const [entryKey, entryValue] of Object.entries(value)) {
              assert2(entryValue === void 0 || typeof entryValue == "number");
              const val = this.enum(enumInfo, entryValue, field.name, false, true, options.enumAsInteger);
              assert2(val !== void 0);
              jsonObj[entryKey.toString()] = val;
            }
            break;
        }
        if (options.emitDefaultValues || Object.keys(jsonObj).length > 0)
          jsonValue = jsonObj;
      } else if (field.repeat) {
        assert2(Array.isArray(value));
        const jsonArr = [];
        switch (field.kind) {
          case "scalar":
            for (let i = 0; i < value.length; i++) {
              const val = this.scalar(field.T, value[i], field.name, field.opt, true);
              assert2(val !== void 0);
              jsonArr.push(val);
            }
            break;
          case "enum":
            const enumInfo = field.T();
            for (let i = 0; i < value.length; i++) {
              assert2(value[i] === void 0 || typeof value[i] == "number");
              const val = this.enum(enumInfo, value[i], field.name, field.opt, true, options.enumAsInteger);
              assert2(val !== void 0);
              jsonArr.push(val);
            }
            break;
          case "message":
            const messageType = field.T();
            for (let i = 0; i < value.length; i++) {
              const val = this.message(messageType, value[i], field.name, options);
              assert2(val !== void 0);
              jsonArr.push(val);
            }
            break;
        }
        if (options.emitDefaultValues || jsonArr.length > 0 || options.emitDefaultValues)
          jsonValue = jsonArr;
      } else {
        switch (field.kind) {
          case "scalar":
            jsonValue = this.scalar(field.T, value, field.name, field.opt, options.emitDefaultValues);
            break;
          case "enum":
            jsonValue = this.enum(field.T(), value, field.name, field.opt, options.emitDefaultValues, options.enumAsInteger);
            break;
          case "message":
            jsonValue = this.message(field.T(), value, field.name, options);
            break;
        }
      }
      return jsonValue;
    }
    /**
     * Returns `null` as the default for google.protobuf.NullValue.
     */
    enum(type, value, fieldName, optional, emitDefaultValues, enumAsInteger) {
      if (type[0] == "google.protobuf.NullValue")
        return !emitDefaultValues && !optional ? void 0 : null;
      if (value === void 0) {
        assert2(optional);
        return void 0;
      }
      if (value === 0 && !emitDefaultValues && !optional)
        return void 0;
      assert2(typeof value == "number");
      assert2(Number.isInteger(value));
      if (enumAsInteger || !type[1].hasOwnProperty(value))
        return value;
      if (type[2])
        return type[2] + type[1][value];
      return type[1][value];
    }
    message(type, value, fieldName, options) {
      if (value === void 0)
        return options.emitDefaultValues ? null : void 0;
      return type.internalJsonWrite(value, options);
    }
    scalar(type, value, fieldName, optional, emitDefaultValues) {
      if (value === void 0) {
        assert2(optional);
        return void 0;
      }
      const ed = emitDefaultValues || optional;
      switch (type) {
        case ScalarType.INT32:
        case ScalarType.SFIXED32:
        case ScalarType.SINT32:
          if (value === 0)
            return ed ? 0 : void 0;
          assertInt322(value);
          return value;
        case ScalarType.FIXED32:
        case ScalarType.UINT32:
          if (value === 0)
            return ed ? 0 : void 0;
          assertUInt322(value);
          return value;
        case ScalarType.FLOAT:
          assertFloat322(value);
        case ScalarType.DOUBLE:
          if (value === 0)
            return ed ? 0 : void 0;
          assert2(typeof value == "number");
          if (Number.isNaN(value))
            return "NaN";
          if (value === Number.POSITIVE_INFINITY)
            return "Infinity";
          if (value === Number.NEGATIVE_INFINITY)
            return "-Infinity";
          return value;
        case ScalarType.STRING:
          if (value === "")
            return ed ? "" : void 0;
          assert2(typeof value == "string");
          return value;
        case ScalarType.BOOL:
          if (value === false)
            return ed ? false : void 0;
          assert2(typeof value == "boolean");
          return value;
        case ScalarType.UINT64:
        case ScalarType.FIXED64:
          assert2(typeof value == "number" || typeof value == "string" || typeof value == "bigint");
          let ulong = PbULong.from(value);
          if (ulong.isZero() && !ed)
            return void 0;
          return ulong.toString();
        case ScalarType.INT64:
        case ScalarType.SFIXED64:
        case ScalarType.SINT64:
          assert2(typeof value == "number" || typeof value == "string" || typeof value == "bigint");
          let long = PbLong.from(value);
          if (long.isZero() && !ed)
            return void 0;
          return long.toString();
        case ScalarType.BYTES:
          assert2(value instanceof Uint8Array);
          if (!value.byteLength)
            return ed ? "" : void 0;
          return base64encode2(value);
      }
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-scalar-default.js
  function reflectionScalarDefault(type, longType = LongType.STRING) {
    switch (type) {
      case ScalarType.BOOL:
        return false;
      case ScalarType.UINT64:
      case ScalarType.FIXED64:
        return reflectionLongConvert(PbULong.ZERO, longType);
      case ScalarType.INT64:
      case ScalarType.SFIXED64:
      case ScalarType.SINT64:
        return reflectionLongConvert(PbLong.ZERO, longType);
      case ScalarType.DOUBLE:
      case ScalarType.FLOAT:
        return 0;
      case ScalarType.BYTES:
        return new Uint8Array(0);
      case ScalarType.STRING:
        return "";
      default:
        return 0;
    }
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-binary-reader.js
  var ReflectionBinaryReader = class {
    constructor(info) {
      this.info = info;
    }
    prepare() {
      var _a;
      if (!this.fieldNoToField) {
        const fieldsInput = (_a = this.info.fields) !== null && _a !== void 0 ? _a : [];
        this.fieldNoToField = new Map(fieldsInput.map((field) => [field.no, field]));
      }
    }
    /**
     * Reads a message from binary format into the target message.
     *
     * Repeated fields are appended. Map entries are added, overwriting
     * existing keys.
     *
     * If a message field is already present, it will be merged with the
     * new data.
     */
    read(reader, message, options, length) {
      this.prepare();
      const end = length === void 0 ? reader.len : reader.pos + length;
      while (reader.pos < end) {
        const [fieldNo, wireType] = reader.tag(), field = this.fieldNoToField.get(fieldNo);
        if (!field) {
          let u = options.readUnknownField;
          if (u == "throw")
            throw new Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.info.typeName}`);
          let d = reader.skip(wireType);
          if (u !== false)
            (u === true ? UnknownFieldHandler2.onRead : u)(this.info.typeName, message, fieldNo, wireType, d);
          continue;
        }
        let target = message, repeated = field.repeat, localName = field.localName;
        if (field.oneof) {
          target = target[field.oneof];
          if (target.oneofKind !== localName)
            target = message[field.oneof] = {
              oneofKind: localName
            };
        }
        switch (field.kind) {
          case "scalar":
          case "enum":
            let T = field.kind == "enum" ? ScalarType.INT32 : field.T;
            let L = field.kind == "scalar" ? field.L : void 0;
            if (repeated) {
              let arr = target[localName];
              if (wireType == WireType2.LengthDelimited && T != ScalarType.STRING && T != ScalarType.BYTES) {
                let e = reader.uint32() + reader.pos;
                while (reader.pos < e)
                  arr.push(this.scalar(reader, T, L));
              } else
                arr.push(this.scalar(reader, T, L));
            } else
              target[localName] = this.scalar(reader, T, L);
            break;
          case "message":
            if (repeated) {
              let arr = target[localName];
              let msg = field.T().internalBinaryRead(reader, reader.uint32(), options);
              arr.push(msg);
            } else
              target[localName] = field.T().internalBinaryRead(reader, reader.uint32(), options, target[localName]);
            break;
          case "map":
            let [mapKey, mapVal] = this.mapEntry(field, reader, options);
            target[localName][mapKey] = mapVal;
            break;
        }
      }
    }
    /**
     * Read a map field, expecting key field = 1, value field = 2
     */
    mapEntry(field, reader, options) {
      let length = reader.uint32();
      let end = reader.pos + length;
      let key = void 0;
      let val = void 0;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case 1:
            if (field.K == ScalarType.BOOL)
              key = reader.bool().toString();
            else
              key = this.scalar(reader, field.K, LongType.STRING);
            break;
          case 2:
            switch (field.V.kind) {
              case "scalar":
                val = this.scalar(reader, field.V.T, field.V.L);
                break;
              case "enum":
                val = reader.int32();
                break;
              case "message":
                val = field.V.T().internalBinaryRead(reader, reader.uint32(), options);
                break;
            }
            break;
          default:
            throw new Error(`Unknown field ${fieldNo} (wire type ${wireType}) in map entry for ${this.info.typeName}#${field.name}`);
        }
      }
      if (key === void 0) {
        let keyRaw = reflectionScalarDefault(field.K);
        key = field.K == ScalarType.BOOL ? keyRaw.toString() : keyRaw;
      }
      if (val === void 0)
        switch (field.V.kind) {
          case "scalar":
            val = reflectionScalarDefault(field.V.T, field.V.L);
            break;
          case "enum":
            val = 0;
            break;
          case "message":
            val = field.V.T().create();
            break;
        }
      return [key, val];
    }
    scalar(reader, type, longType) {
      switch (type) {
        case ScalarType.INT32:
          return reader.int32();
        case ScalarType.STRING:
          return reader.string();
        case ScalarType.BOOL:
          return reader.bool();
        case ScalarType.DOUBLE:
          return reader.double();
        case ScalarType.FLOAT:
          return reader.float();
        case ScalarType.INT64:
          return reflectionLongConvert(reader.int64(), longType);
        case ScalarType.UINT64:
          return reflectionLongConvert(reader.uint64(), longType);
        case ScalarType.FIXED64:
          return reflectionLongConvert(reader.fixed64(), longType);
        case ScalarType.FIXED32:
          return reader.fixed32();
        case ScalarType.BYTES:
          return reader.bytes();
        case ScalarType.UINT32:
          return reader.uint32();
        case ScalarType.SFIXED32:
          return reader.sfixed32();
        case ScalarType.SFIXED64:
          return reflectionLongConvert(reader.sfixed64(), longType);
        case ScalarType.SINT32:
          return reader.sint32();
        case ScalarType.SINT64:
          return reflectionLongConvert(reader.sint64(), longType);
      }
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-binary-writer.js
  var ReflectionBinaryWriter = class {
    constructor(info) {
      this.info = info;
    }
    prepare() {
      if (!this.fields) {
        const fieldsInput = this.info.fields ? this.info.fields.concat() : [];
        this.fields = fieldsInput.sort((a, b) => a.no - b.no);
      }
    }
    /**
     * Writes the message to binary format.
     */
    write(message, writer, options) {
      this.prepare();
      for (const field of this.fields) {
        let value, emitDefault, repeated = field.repeat, localName = field.localName;
        if (field.oneof) {
          const group = message[field.oneof];
          if (group.oneofKind !== localName)
            continue;
          value = group[localName];
          emitDefault = true;
        } else {
          value = message[localName];
          emitDefault = false;
        }
        switch (field.kind) {
          case "scalar":
          case "enum":
            let T = field.kind == "enum" ? ScalarType.INT32 : field.T;
            if (repeated) {
              assert2(Array.isArray(value));
              if (repeated == RepeatType.PACKED)
                this.packed(writer, T, field.no, value);
              else
                for (const item of value)
                  this.scalar(writer, T, field.no, item, true);
            } else if (value === void 0)
              assert2(field.opt);
            else
              this.scalar(writer, T, field.no, value, emitDefault || field.opt);
            break;
          case "message":
            if (repeated) {
              assert2(Array.isArray(value));
              for (const item of value)
                this.message(writer, options, field.T(), field.no, item);
            } else {
              this.message(writer, options, field.T(), field.no, value);
            }
            break;
          case "map":
            assert2(typeof value == "object" && value !== null);
            for (const [key, val] of Object.entries(value))
              this.mapEntry(writer, options, field, key, val);
            break;
        }
      }
      let u = options.writeUnknownFields;
      if (u !== false)
        (u === true ? UnknownFieldHandler2.onWrite : u)(this.info.typeName, message, writer);
    }
    mapEntry(writer, options, field, key, value) {
      writer.tag(field.no, WireType2.LengthDelimited);
      writer.fork();
      let keyValue = key;
      switch (field.K) {
        case ScalarType.INT32:
        case ScalarType.FIXED32:
        case ScalarType.UINT32:
        case ScalarType.SFIXED32:
        case ScalarType.SINT32:
          keyValue = Number.parseInt(key);
          break;
        case ScalarType.BOOL:
          assert2(key == "true" || key == "false");
          keyValue = key == "true";
          break;
      }
      this.scalar(writer, field.K, 1, keyValue, true);
      switch (field.V.kind) {
        case "scalar":
          this.scalar(writer, field.V.T, 2, value, true);
          break;
        case "enum":
          this.scalar(writer, ScalarType.INT32, 2, value, true);
          break;
        case "message":
          this.message(writer, options, field.V.T(), 2, value);
          break;
      }
      writer.join();
    }
    message(writer, options, handler, fieldNo, value) {
      if (value === void 0)
        return;
      handler.internalBinaryWrite(value, writer.tag(fieldNo, WireType2.LengthDelimited).fork(), options);
      writer.join();
    }
    /**
     * Write a single scalar value.
     */
    scalar(writer, type, fieldNo, value, emitDefault) {
      let [wireType, method, isDefault] = this.scalarInfo(type, value);
      if (!isDefault || emitDefault) {
        writer.tag(fieldNo, wireType);
        writer[method](value);
      }
    }
    /**
     * Write an array of scalar values in packed format.
     */
    packed(writer, type, fieldNo, value) {
      if (!value.length)
        return;
      assert2(type !== ScalarType.BYTES && type !== ScalarType.STRING);
      writer.tag(fieldNo, WireType2.LengthDelimited);
      writer.fork();
      let [, method] = this.scalarInfo(type);
      for (let i = 0; i < value.length; i++)
        writer[method](value[i]);
      writer.join();
    }
    /**
     * Get information for writing a scalar value.
     *
     * Returns tuple:
     * [0]: appropriate WireType
     * [1]: name of the appropriate method of IBinaryWriter
     * [2]: whether the given value is a default value
     *
     * If argument `value` is omitted, [2] is always false.
     */
    scalarInfo(type, value) {
      let t = WireType2.Varint;
      let m;
      let i = value === void 0;
      let d = value === 0;
      switch (type) {
        case ScalarType.INT32:
          m = "int32";
          break;
        case ScalarType.STRING:
          d = i || !value.length;
          t = WireType2.LengthDelimited;
          m = "string";
          break;
        case ScalarType.BOOL:
          d = value === false;
          m = "bool";
          break;
        case ScalarType.UINT32:
          m = "uint32";
          break;
        case ScalarType.DOUBLE:
          t = WireType2.Bit64;
          m = "double";
          break;
        case ScalarType.FLOAT:
          t = WireType2.Bit32;
          m = "float";
          break;
        case ScalarType.INT64:
          d = i || PbLong.from(value).isZero();
          m = "int64";
          break;
        case ScalarType.UINT64:
          d = i || PbULong.from(value).isZero();
          m = "uint64";
          break;
        case ScalarType.FIXED64:
          d = i || PbULong.from(value).isZero();
          t = WireType2.Bit64;
          m = "fixed64";
          break;
        case ScalarType.BYTES:
          d = i || !value.byteLength;
          t = WireType2.LengthDelimited;
          m = "bytes";
          break;
        case ScalarType.FIXED32:
          t = WireType2.Bit32;
          m = "fixed32";
          break;
        case ScalarType.SFIXED32:
          t = WireType2.Bit32;
          m = "sfixed32";
          break;
        case ScalarType.SFIXED64:
          d = i || PbLong.from(value).isZero();
          t = WireType2.Bit64;
          m = "sfixed64";
          break;
        case ScalarType.SINT32:
          m = "sint32";
          break;
        case ScalarType.SINT64:
          d = i || PbLong.from(value).isZero();
          m = "sint64";
          break;
      }
      return [t, m, i || d];
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-create.js
  function reflectionCreate(type) {
    const msg = type.messagePrototype ? Object.create(type.messagePrototype) : Object.defineProperty({}, MESSAGE_TYPE, { value: type });
    for (let field of type.fields) {
      let name = field.localName;
      if (field.opt)
        continue;
      if (field.oneof)
        msg[field.oneof] = { oneofKind: void 0 };
      else if (field.repeat)
        msg[name] = [];
      else
        switch (field.kind) {
          case "scalar":
            msg[name] = reflectionScalarDefault(field.T, field.L);
            break;
          case "enum":
            msg[name] = 0;
            break;
          case "map":
            msg[name] = {};
            break;
        }
    }
    return msg;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-merge-partial.js
  function reflectionMergePartial(info, target, source) {
    let fieldValue, input = source, output;
    for (let field of info.fields) {
      let name = field.localName;
      if (field.oneof) {
        const group = input[field.oneof];
        if ((group === null || group === void 0 ? void 0 : group.oneofKind) == void 0) {
          continue;
        }
        fieldValue = group[name];
        output = target[field.oneof];
        output.oneofKind = group.oneofKind;
        if (fieldValue == void 0) {
          delete output[name];
          continue;
        }
      } else {
        fieldValue = input[name];
        output = target;
        if (fieldValue == void 0) {
          continue;
        }
      }
      if (field.repeat)
        output[name].length = fieldValue.length;
      switch (field.kind) {
        case "scalar":
        case "enum":
          if (field.repeat)
            for (let i = 0; i < fieldValue.length; i++)
              output[name][i] = fieldValue[i];
          else
            output[name] = fieldValue;
          break;
        case "message":
          let T = field.T();
          if (field.repeat)
            for (let i = 0; i < fieldValue.length; i++)
              output[name][i] = T.create(fieldValue[i]);
          else if (output[name] === void 0)
            output[name] = T.create(fieldValue);
          else
            T.mergePartial(output[name], fieldValue);
          break;
        case "map":
          switch (field.V.kind) {
            case "scalar":
            case "enum":
              Object.assign(output[name], fieldValue);
              break;
            case "message":
              let T2 = field.V.T();
              for (let k of Object.keys(fieldValue))
                output[name][k] = T2.create(fieldValue[k]);
              break;
          }
          break;
      }
    }
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/reflection-equals.js
  function reflectionEquals(info, a, b) {
    if (a === b)
      return true;
    if (!a || !b)
      return false;
    for (let field of info.fields) {
      let localName = field.localName;
      let val_a = field.oneof ? a[field.oneof][localName] : a[localName];
      let val_b = field.oneof ? b[field.oneof][localName] : b[localName];
      switch (field.kind) {
        case "enum":
        case "scalar":
          let t = field.kind == "enum" ? ScalarType.INT32 : field.T;
          if (!(field.repeat ? repeatedPrimitiveEq(t, val_a, val_b) : primitiveEq(t, val_a, val_b)))
            return false;
          break;
        case "map":
          if (!(field.V.kind == "message" ? repeatedMsgEq(field.V.T(), objectValues(val_a), objectValues(val_b)) : repeatedPrimitiveEq(field.V.kind == "enum" ? ScalarType.INT32 : field.V.T, objectValues(val_a), objectValues(val_b))))
            return false;
          break;
        case "message":
          let T = field.T();
          if (!(field.repeat ? repeatedMsgEq(T, val_a, val_b) : T.equals(val_a, val_b)))
            return false;
          break;
      }
    }
    return true;
  }
  var objectValues = Object.values;
  function primitiveEq(type, a, b) {
    if (a === b)
      return true;
    if (type !== ScalarType.BYTES)
      return false;
    let ba = a;
    let bb = b;
    if (ba.length !== bb.length)
      return false;
    for (let i = 0; i < ba.length; i++)
      if (ba[i] != bb[i])
        return false;
    return true;
  }
  function repeatedPrimitiveEq(type, a, b) {
    if (a.length !== b.length)
      return false;
    for (let i = 0; i < a.length; i++)
      if (!primitiveEq(type, a[i], b[i]))
        return false;
    return true;
  }
  function repeatedMsgEq(type, a, b) {
    if (a.length !== b.length)
      return false;
    for (let i = 0; i < a.length; i++)
      if (!type.equals(a[i], b[i]))
        return false;
    return true;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime/build/es2015/message-type.js
  var baseDescriptors = Object.getOwnPropertyDescriptors(Object.getPrototypeOf({}));
  var messageTypeDescriptor = baseDescriptors[MESSAGE_TYPE] = {};
  var MessageType = class {
    constructor(name, fields, options) {
      this.defaultCheckDepth = 16;
      this.typeName = name;
      this.fields = fields.map(normalizeFieldInfo);
      this.options = options !== null && options !== void 0 ? options : {};
      messageTypeDescriptor.value = this;
      this.messagePrototype = Object.create(null, baseDescriptors);
      this.refTypeCheck = new ReflectionTypeCheck(this);
      this.refJsonReader = new ReflectionJsonReader(this);
      this.refJsonWriter = new ReflectionJsonWriter(this);
      this.refBinReader = new ReflectionBinaryReader(this);
      this.refBinWriter = new ReflectionBinaryWriter(this);
    }
    create(value) {
      let message = reflectionCreate(this);
      if (value !== void 0) {
        reflectionMergePartial(this, message, value);
      }
      return message;
    }
    /**
     * Clone the message.
     *
     * Unknown fields are discarded.
     */
    clone(message) {
      let copy2 = this.create();
      reflectionMergePartial(this, copy2, message);
      return copy2;
    }
    /**
     * Determines whether two message of the same type have the same field values.
     * Checks for deep equality, traversing repeated fields, oneof groups, maps
     * and messages recursively.
     * Will also return true if both messages are `undefined`.
     */
    equals(a, b) {
      return reflectionEquals(this, a, b);
    }
    /**
     * Is the given value assignable to our message type
     * and contains no [excess properties](https://www.typescriptlang.org/docs/handbook/interfaces.html#excess-property-checks)?
     */
    is(arg, depth = this.defaultCheckDepth) {
      return this.refTypeCheck.is(arg, depth, false);
    }
    /**
     * Is the given value assignable to our message type,
     * regardless of [excess properties](https://www.typescriptlang.org/docs/handbook/interfaces.html#excess-property-checks)?
     */
    isAssignable(arg, depth = this.defaultCheckDepth) {
      return this.refTypeCheck.is(arg, depth, true);
    }
    /**
     * Copy partial data into the target message.
     */
    mergePartial(target, source) {
      reflectionMergePartial(this, target, source);
    }
    /**
     * Create a new message from binary format.
     */
    fromBinary(data, options) {
      let opt = binaryReadOptions(options);
      return this.internalBinaryRead(opt.readerFactory(data), data.byteLength, opt);
    }
    /**
     * Read a new message from a JSON value.
     */
    fromJson(json, options) {
      return this.internalJsonRead(json, jsonReadOptions2(options));
    }
    /**
     * Read a new message from a JSON string.
     * This is equivalent to `T.fromJson(JSON.parse(json))`.
     */
    fromJsonString(json, options) {
      let value = JSON.parse(json);
      return this.fromJson(value, options);
    }
    /**
     * Write the message to canonical JSON value.
     */
    toJson(message, options) {
      return this.internalJsonWrite(message, jsonWriteOptions2(options));
    }
    /**
     * Convert the message to canonical JSON string.
     * This is equivalent to `JSON.stringify(T.toJson(t))`
     */
    toJsonString(message, options) {
      var _a;
      let value = this.toJson(message, options);
      return JSON.stringify(value, null, (_a = options === null || options === void 0 ? void 0 : options.prettySpaces) !== null && _a !== void 0 ? _a : 0);
    }
    /**
     * Write the message to binary format.
     */
    toBinary(message, options) {
      let opt = binaryWriteOptions(options);
      return this.internalBinaryWrite(message, opt.writerFactory(), opt).finish();
    }
    /**
     * This is an internal method. If you just want to read a message from
     * JSON, use `fromJson()` or `fromJsonString()`.
     *
     * Reads JSON value and merges the fields into the target
     * according to protobuf rules. If the target is omitted,
     * a new instance is created first.
     */
    internalJsonRead(json, options, target) {
      if (json !== null && typeof json == "object" && !Array.isArray(json)) {
        let message = target !== null && target !== void 0 ? target : this.create();
        this.refJsonReader.read(json, message, options);
        return message;
      }
      throw new Error(`Unable to parse message ${this.typeName} from JSON ${typeofJsonValue(json)}.`);
    }
    /**
     * This is an internal method. If you just want to write a message
     * to JSON, use `toJson()` or `toJsonString().
     *
     * Writes JSON value and returns it.
     */
    internalJsonWrite(message, options) {
      return this.refJsonWriter.write(message, options);
    }
    /**
     * This is an internal method. If you just want to write a message
     * in binary format, use `toBinary()`.
     *
     * Serializes the message in binary format and appends it to the given
     * writer. Returns passed writer.
     */
    internalBinaryWrite(message, writer, options) {
      this.refBinWriter.write(message, writer, options);
      return writer;
    }
    /**
     * This is an internal method. If you just want to read a message from
     * binary data, use `fromBinary()`.
     *
     * Reads data from binary format and merges the fields into
     * the target according to protobuf rules. If the target is
     * omitted, a new instance is created first.
     */
    internalBinaryRead(reader, length, options, target) {
      let message = target !== null && target !== void 0 ? target : this.create();
      this.refBinReader.read(reader, message, options, length);
      return message;
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime-rpc/build/es2015/reflection-info.js
  function normalizeMethodInfo(method, service) {
    var _a, _b, _c;
    let m = method;
    m.service = service;
    m.localName = (_a = m.localName) !== null && _a !== void 0 ? _a : lowerCamelCase(m.name);
    m.serverStreaming = !!m.serverStreaming;
    m.clientStreaming = !!m.clientStreaming;
    m.options = (_b = m.options) !== null && _b !== void 0 ? _b : {};
    m.idempotency = (_c = m.idempotency) !== null && _c !== void 0 ? _c : void 0;
    return m;
  }

  // ../frontend/node_modules/@protobuf-ts/runtime-rpc/build/es2015/service-type.js
  var ServiceType = class {
    constructor(typeName, methods, options) {
      this.typeName = typeName;
      this.methods = methods.map((i) => normalizeMethodInfo(i, this));
      this.options = options !== null && options !== void 0 ? options : {};
    }
  };

  // ../frontend/node_modules/@protobuf-ts/runtime-rpc/build/es2015/rpc-interceptor.js
  function stackIntercept(kind, transport, method, options, input) {
    var _a, _b, _c, _d;
    if (kind == "unary") {
      let tail = (mtd, inp, opt) => transport.unary(mtd, inp, opt);
      for (const curr of ((_a = options.interceptors) !== null && _a !== void 0 ? _a : []).filter((i) => i.interceptUnary).reverse()) {
        const next = tail;
        tail = (mtd, inp, opt) => curr.interceptUnary(next, mtd, inp, opt);
      }
      return tail(method, input, options);
    }
    if (kind == "serverStreaming") {
      let tail = (mtd, inp, opt) => transport.serverStreaming(mtd, inp, opt);
      for (const curr of ((_b = options.interceptors) !== null && _b !== void 0 ? _b : []).filter((i) => i.interceptServerStreaming).reverse()) {
        const next = tail;
        tail = (mtd, inp, opt) => curr.interceptServerStreaming(next, mtd, inp, opt);
      }
      return tail(method, input, options);
    }
    if (kind == "clientStreaming") {
      let tail = (mtd, opt) => transport.clientStreaming(mtd, opt);
      for (const curr of ((_c = options.interceptors) !== null && _c !== void 0 ? _c : []).filter((i) => i.interceptClientStreaming).reverse()) {
        const next = tail;
        tail = (mtd, opt) => curr.interceptClientStreaming(next, mtd, opt);
      }
      return tail(method, options);
    }
    if (kind == "duplex") {
      let tail = (mtd, opt) => transport.duplex(mtd, opt);
      for (const curr of ((_d = options.interceptors) !== null && _d !== void 0 ? _d : []).filter((i) => i.interceptDuplex).reverse()) {
        const next = tail;
        tail = (mtd, opt) => curr.interceptDuplex(next, mtd, opt);
      }
      return tail(method, options);
    }
    assertNever2(kind);
  }

  // ../frontend/src/generated/common.ts
  var SessionState = /* @__PURE__ */ ((SessionState2) => {
    SessionState2[SessionState2["WAITING"] = 0] = "WAITING";
    SessionState2[SessionState2["IN_PROGRESS"] = 1] = "IN_PROGRESS";
    SessionState2[SessionState2["FINISHED"] = 2] = "FINISHED";
    SessionState2[SessionState2["CANCELLED"] = 3] = "CANCELLED";
    return SessionState2;
  })(SessionState || {});
  var Player$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.Player", [
        {
          no: 1,
          name: "id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "name",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "score",
          kind: "scalar",
          T: 5
          /*ScalarType.INT32*/
        },
        {
          no: 4,
          name: "is_ready",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        },
        {
          no: 5,
          name: "is_connected",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        },
        {
          no: 6,
          name: "joined_at",
          kind: "scalar",
          T: 3,
          L: 0
          /*LongType.BIGINT*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.id = "";
      message.name = "";
      message.score = 0;
      message.isReady = false;
      message.isConnected = false;
      message.joinedAt = 0n;
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string id */
          1:
            message.id = reader.string();
            break;
          case /* string name */
          2:
            message.name = reader.string();
            break;
          case /* int32 score */
          3:
            message.score = reader.int32();
            break;
          case /* bool is_ready */
          4:
            message.isReady = reader.bool();
            break;
          case /* bool is_connected */
          5:
            message.isConnected = reader.bool();
            break;
          case /* int64 joined_at */
          6:
            message.joinedAt = reader.int64().toBigInt();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.id !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.id);
      if (message.name !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.name);
      if (message.score !== 0)
        writer.tag(3, WireType2.Varint).int32(message.score);
      if (message.isReady !== false)
        writer.tag(4, WireType2.Varint).bool(message.isReady);
      if (message.isConnected !== false)
        writer.tag(5, WireType2.Varint).bool(message.isConnected);
      if (message.joinedAt !== 0n)
        writer.tag(6, WireType2.Varint).int64(message.joinedAt);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var Player = new Player$Type();
  var GameState$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GameState", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 2, name: "players", kind: "message", repeat: 2, T: () => Player },
        {
          no: 3,
          name: "current_player_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 4, name: "state", kind: "enum", T: () => ["takeiteasygame.v1.SessionState", SessionState] },
        {
          no: 5,
          name: "board_state",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 6,
          name: "turn_number",
          kind: "scalar",
          T: 5
          /*ScalarType.INT32*/
        },
        {
          no: 7,
          name: "game_mode",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      message.players = [];
      message.currentPlayerId = "";
      message.state = 0;
      message.boardState = "";
      message.turnNumber = 0;
      message.gameMode = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          case /* repeated takeiteasygame.v1.Player players */
          2:
            message.players.push(Player.internalBinaryRead(reader, reader.uint32(), options));
            break;
          case /* string current_player_id */
          3:
            message.currentPlayerId = reader.string();
            break;
          case /* takeiteasygame.v1.SessionState state */
          4:
            message.state = reader.int32();
            break;
          case /* string board_state */
          5:
            message.boardState = reader.string();
            break;
          case /* int32 turn_number */
          6:
            message.turnNumber = reader.int32();
            break;
          case /* string game_mode */
          7:
            message.gameMode = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      for (let i = 0; i < message.players.length; i++)
        Player.internalBinaryWrite(message.players[i], writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      if (message.currentPlayerId !== "")
        writer.tag(3, WireType2.LengthDelimited).string(message.currentPlayerId);
      if (message.state !== 0)
        writer.tag(4, WireType2.Varint).int32(message.state);
      if (message.boardState !== "")
        writer.tag(5, WireType2.LengthDelimited).string(message.boardState);
      if (message.turnNumber !== 0)
        writer.tag(6, WireType2.Varint).int32(message.turnNumber);
      if (message.gameMode !== "")
        writer.tag(7, WireType2.LengthDelimited).string(message.gameMode);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GameState = new GameState$Type();
  var Error$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.Error", [
        {
          no: 1,
          name: "code",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "message",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 3, name: "details", kind: "map", K: 9, V: {
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        } }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.code = "";
      message.message = "";
      message.details = {};
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string code */
          1:
            message.code = reader.string();
            break;
          case /* string message */
          2:
            message.message = reader.string();
            break;
          case /* map<string, string> details */
          3:
            this.binaryReadMap3(message.details, reader, options);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    binaryReadMap3(map, reader, options) {
      let len = reader.uint32(), end = reader.pos + len, key, val;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case 1:
            key = reader.string();
            break;
          case 2:
            val = reader.string();
            break;
          default:
            throw new globalThis.Error("unknown map entry field for takeiteasygame.v1.Error.details");
        }
      }
      map[key ?? ""] = val ?? "";
    }
    internalBinaryWrite(message, writer, options) {
      if (message.code !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.code);
      if (message.message !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.message);
      for (let k of globalThis.Object.keys(message.details))
        writer.tag(3, WireType2.LengthDelimited).fork().tag(1, WireType2.LengthDelimited).string(k).tag(2, WireType2.LengthDelimited).string(message.details[k]).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var Error2 = new Error$Type();

  // ../frontend/src/generated/session_service.ts
  var CreateSessionRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.CreateSessionRequest", [
        {
          no: 1,
          name: "player_name",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "max_players",
          kind: "scalar",
          T: 5
          /*ScalarType.INT32*/
        },
        {
          no: 3,
          name: "game_mode",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.playerName = "";
      message.maxPlayers = 0;
      message.gameMode = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string player_name */
          1:
            message.playerName = reader.string();
            break;
          case /* int32 max_players */
          2:
            message.maxPlayers = reader.int32();
            break;
          case /* string game_mode */
          3:
            message.gameMode = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.playerName !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.playerName);
      if (message.maxPlayers !== 0)
        writer.tag(2, WireType2.Varint).int32(message.maxPlayers);
      if (message.gameMode !== "")
        writer.tag(3, WireType2.LengthDelimited).string(message.gameMode);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var CreateSessionRequest = new CreateSessionRequest$Type();
  var CreateSessionSuccess$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.CreateSessionSuccess", [
        {
          no: 1,
          name: "session_code",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "player_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 4, name: "player", kind: "message", T: () => Player }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionCode = "";
      message.sessionId = "";
      message.playerId = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_code */
          1:
            message.sessionCode = reader.string();
            break;
          case /* string session_id */
          2:
            message.sessionId = reader.string();
            break;
          case /* string player_id */
          3:
            message.playerId = reader.string();
            break;
          case /* takeiteasygame.v1.Player player */
          4:
            message.player = Player.internalBinaryRead(reader, reader.uint32(), options, message.player);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionCode !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionCode);
      if (message.sessionId !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.sessionId);
      if (message.playerId !== "")
        writer.tag(3, WireType2.LengthDelimited).string(message.playerId);
      if (message.player)
        Player.internalBinaryWrite(message.player, writer.tag(4, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var CreateSessionSuccess = new CreateSessionSuccess$Type();
  var CreateSessionResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.CreateSessionResponse", [
        { no: 1, name: "success", kind: "message", oneof: "result", T: () => CreateSessionSuccess },
        { no: 2, name: "error", kind: "message", oneof: "result", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.result = { oneofKind: void 0 };
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* takeiteasygame.v1.CreateSessionSuccess success */
          1:
            message.result = {
              oneofKind: "success",
              success: CreateSessionSuccess.internalBinaryRead(reader, reader.uint32(), options, message.result.success)
            };
            break;
          case /* takeiteasygame.v1.Error error */
          2:
            message.result = {
              oneofKind: "error",
              error: Error2.internalBinaryRead(reader, reader.uint32(), options, message.result.error)
            };
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.result.oneofKind === "success")
        CreateSessionSuccess.internalBinaryWrite(message.result.success, writer.tag(1, WireType2.LengthDelimited).fork(), options).join();
      if (message.result.oneofKind === "error")
        Error2.internalBinaryWrite(message.result.error, writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var CreateSessionResponse = new CreateSessionResponse$Type();
  var JoinSessionRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.JoinSessionRequest", [
        {
          no: 1,
          name: "session_code",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "player_name",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionCode = "";
      message.playerName = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_code */
          1:
            message.sessionCode = reader.string();
            break;
          case /* string player_name */
          2:
            message.playerName = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionCode !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionCode);
      if (message.playerName !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.playerName);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var JoinSessionRequest = new JoinSessionRequest$Type();
  var JoinSessionSuccess$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.JoinSessionSuccess", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "player_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 3, name: "game_state", kind: "message", T: () => GameState }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      message.playerId = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          case /* string player_id */
          2:
            message.playerId = reader.string();
            break;
          case /* takeiteasygame.v1.GameState game_state */
          3:
            message.gameState = GameState.internalBinaryRead(reader, reader.uint32(), options, message.gameState);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      if (message.playerId !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.playerId);
      if (message.gameState)
        GameState.internalBinaryWrite(message.gameState, writer.tag(3, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var JoinSessionSuccess = new JoinSessionSuccess$Type();
  var JoinSessionResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.JoinSessionResponse", [
        { no: 1, name: "success", kind: "message", oneof: "result", T: () => JoinSessionSuccess },
        { no: 2, name: "error", kind: "message", oneof: "result", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.result = { oneofKind: void 0 };
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* takeiteasygame.v1.JoinSessionSuccess success */
          1:
            message.result = {
              oneofKind: "success",
              success: JoinSessionSuccess.internalBinaryRead(reader, reader.uint32(), options, message.result.success)
            };
            break;
          case /* takeiteasygame.v1.Error error */
          2:
            message.result = {
              oneofKind: "error",
              error: Error2.internalBinaryRead(reader, reader.uint32(), options, message.result.error)
            };
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.result.oneofKind === "success")
        JoinSessionSuccess.internalBinaryWrite(message.result.success, writer.tag(1, WireType2.LengthDelimited).fork(), options).join();
      if (message.result.oneofKind === "error")
        Error2.internalBinaryWrite(message.result.error, writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var JoinSessionResponse = new JoinSessionResponse$Type();
  var SetReadyRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.SetReadyRequest", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "player_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "ready",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      message.playerId = "";
      message.ready = false;
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          case /* string player_id */
          2:
            message.playerId = reader.string();
            break;
          case /* bool ready */
          3:
            message.ready = reader.bool();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      if (message.playerId !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.playerId);
      if (message.ready !== false)
        writer.tag(3, WireType2.Varint).bool(message.ready);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var SetReadyRequest = new SetReadyRequest$Type();
  var SetReadyResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.SetReadyResponse", [
        {
          no: 1,
          name: "success",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        },
        { no: 2, name: "error", kind: "message", T: () => Error2 },
        {
          no: 3,
          name: "game_started",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.success = false;
      message.gameStarted = false;
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* bool success */
          1:
            message.success = reader.bool();
            break;
          case /* takeiteasygame.v1.Error error */
          2:
            message.error = Error2.internalBinaryRead(reader, reader.uint32(), options, message.error);
            break;
          case /* bool game_started */
          3:
            message.gameStarted = reader.bool();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.success !== false)
        writer.tag(1, WireType2.Varint).bool(message.success);
      if (message.error)
        Error2.internalBinaryWrite(message.error, writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      if (message.gameStarted !== false)
        writer.tag(3, WireType2.Varint).bool(message.gameStarted);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var SetReadyResponse = new SetReadyResponse$Type();
  var GetSessionStateRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GetSessionStateRequest", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GetSessionStateRequest = new GetSessionStateRequest$Type();
  var GetSessionStateResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GetSessionStateResponse", [
        { no: 1, name: "game_state", kind: "message", T: () => GameState },
        { no: 2, name: "error", kind: "message", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* takeiteasygame.v1.GameState game_state */
          1:
            message.gameState = GameState.internalBinaryRead(reader, reader.uint32(), options, message.gameState);
            break;
          case /* takeiteasygame.v1.Error error */
          2:
            message.error = Error2.internalBinaryRead(reader, reader.uint32(), options, message.error);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.gameState)
        GameState.internalBinaryWrite(message.gameState, writer.tag(1, WireType2.LengthDelimited).fork(), options).join();
      if (message.error)
        Error2.internalBinaryWrite(message.error, writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GetSessionStateResponse = new GetSessionStateResponse$Type();
  var SessionService = new ServiceType("takeiteasygame.v1.SessionService", [
    { name: "CreateSession", options: {}, I: CreateSessionRequest, O: CreateSessionResponse },
    { name: "JoinSession", options: {}, I: JoinSessionRequest, O: JoinSessionResponse },
    { name: "SetReady", options: {}, I: SetReadyRequest, O: SetReadyResponse },
    { name: "GetSessionState", options: {}, I: GetSessionStateRequest, O: GetSessionStateResponse }
  ]);

  // ../frontend/src/generated/session_service.client.ts
  var SessionServiceClient = class {
    constructor(_transport) {
      this._transport = _transport;
    }
    typeName = SessionService.typeName;
    methods = SessionService.methods;
    options = SessionService.options;
    /**
     * @generated from protobuf rpc: CreateSession
     */
    createSession(input, options) {
      const method = this.methods[0], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
    /**
     * @generated from protobuf rpc: JoinSession
     */
    joinSession(input, options) {
      const method = this.methods[1], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
    /**
     * @generated from protobuf rpc: SetReady
     */
    setReady(input, options) {
      const method = this.methods[2], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
    /**
     * @generated from protobuf rpc: GetSessionState
     */
    getSessionState(input, options) {
      const method = this.methods[3], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
  };

  // ../frontend/src/generated/game_service.ts
  var MakeMoveRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.MakeMoveRequest", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "player_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "move_data",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 4,
          name: "timestamp",
          kind: "scalar",
          T: 3,
          L: 0
          /*LongType.BIGINT*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      message.playerId = "";
      message.moveData = "";
      message.timestamp = 0n;
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          case /* string player_id */
          2:
            message.playerId = reader.string();
            break;
          case /* string move_data */
          3:
            message.moveData = reader.string();
            break;
          case /* int64 timestamp */
          4:
            message.timestamp = reader.int64().toBigInt();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      if (message.playerId !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.playerId);
      if (message.moveData !== "")
        writer.tag(3, WireType2.LengthDelimited).string(message.moveData);
      if (message.timestamp !== 0n)
        writer.tag(4, WireType2.Varint).int64(message.timestamp);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var MakeMoveRequest = new MakeMoveRequest$Type();
  var MakeMoveResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.MakeMoveResponse", [
        { no: 1, name: "success", kind: "message", oneof: "result", T: () => MakeMoveSuccess },
        { no: 2, name: "error", kind: "message", oneof: "result", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.result = { oneofKind: void 0 };
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* takeiteasygame.v1.MakeMoveSuccess success */
          1:
            message.result = {
              oneofKind: "success",
              success: MakeMoveSuccess.internalBinaryRead(reader, reader.uint32(), options, message.result.success)
            };
            break;
          case /* takeiteasygame.v1.Error error */
          2:
            message.result = {
              oneofKind: "error",
              error: Error2.internalBinaryRead(reader, reader.uint32(), options, message.result.error)
            };
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.result.oneofKind === "success")
        MakeMoveSuccess.internalBinaryWrite(message.result.success, writer.tag(1, WireType2.LengthDelimited).fork(), options).join();
      if (message.result.oneofKind === "error")
        Error2.internalBinaryWrite(message.result.error, writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var MakeMoveResponse = new MakeMoveResponse$Type();
  var MakeMoveSuccess$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.MakeMoveSuccess", [
        { no: 1, name: "new_game_state", kind: "message", T: () => GameState },
        {
          no: 2,
          name: "mcts_response",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "points_earned",
          kind: "scalar",
          T: 5
          /*ScalarType.INT32*/
        },
        {
          no: 4,
          name: "is_game_over",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.mctsResponse = "";
      message.pointsEarned = 0;
      message.isGameOver = false;
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* takeiteasygame.v1.GameState new_game_state */
          1:
            message.newGameState = GameState.internalBinaryRead(reader, reader.uint32(), options, message.newGameState);
            break;
          case /* string mcts_response */
          2:
            message.mctsResponse = reader.string();
            break;
          case /* int32 points_earned */
          3:
            message.pointsEarned = reader.int32();
            break;
          case /* bool is_game_over */
          4:
            message.isGameOver = reader.bool();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.newGameState)
        GameState.internalBinaryWrite(message.newGameState, writer.tag(1, WireType2.LengthDelimited).fork(), options).join();
      if (message.mctsResponse !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.mctsResponse);
      if (message.pointsEarned !== 0)
        writer.tag(3, WireType2.Varint).int32(message.pointsEarned);
      if (message.isGameOver !== false)
        writer.tag(4, WireType2.Varint).bool(message.isGameOver);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var MakeMoveSuccess = new MakeMoveSuccess$Type();
  var GetAvailableMovesRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GetAvailableMovesRequest", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 2,
          name: "player_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      message.playerId = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          case /* string player_id */
          2:
            message.playerId = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      if (message.playerId !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.playerId);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GetAvailableMovesRequest = new GetAvailableMovesRequest$Type();
  var GetAvailableMovesResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GetAvailableMovesResponse", [
        {
          no: 1,
          name: "available_moves",
          kind: "scalar",
          repeat: 2,
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 2, name: "error", kind: "message", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.availableMoves = [];
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* repeated string available_moves */
          1:
            message.availableMoves.push(reader.string());
            break;
          case /* takeiteasygame.v1.Error error */
          2:
            message.error = Error2.internalBinaryRead(reader, reader.uint32(), options, message.error);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      for (let i = 0; i < message.availableMoves.length; i++)
        writer.tag(1, WireType2.LengthDelimited).string(message.availableMoves[i]);
      if (message.error)
        Error2.internalBinaryWrite(message.error, writer.tag(2, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GetAvailableMovesResponse = new GetAvailableMovesResponse$Type();
  var StartTurnRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.StartTurnRequest", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var StartTurnRequest = new StartTurnRequest$Type();
  var StartTurnResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.StartTurnResponse", [
        {
          no: 1,
          name: "success",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        },
        {
          no: 2,
          name: "announced_tile",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "tile_image",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 4,
          name: "turn_number",
          kind: "scalar",
          T: 5
          /*ScalarType.INT32*/
        },
        {
          no: 5,
          name: "waiting_for_players",
          kind: "scalar",
          repeat: 2,
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 6,
          name: "game_state",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 7, name: "error", kind: "message", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.success = false;
      message.announcedTile = "";
      message.tileImage = "";
      message.turnNumber = 0;
      message.waitingForPlayers = [];
      message.gameState = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* bool success */
          1:
            message.success = reader.bool();
            break;
          case /* string announced_tile */
          2:
            message.announcedTile = reader.string();
            break;
          case /* string tile_image */
          3:
            message.tileImage = reader.string();
            break;
          case /* int32 turn_number */
          4:
            message.turnNumber = reader.int32();
            break;
          case /* repeated string waiting_for_players */
          5:
            message.waitingForPlayers.push(reader.string());
            break;
          case /* string game_state */
          6:
            message.gameState = reader.string();
            break;
          case /* takeiteasygame.v1.Error error */
          7:
            message.error = Error2.internalBinaryRead(reader, reader.uint32(), options, message.error);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.success !== false)
        writer.tag(1, WireType2.Varint).bool(message.success);
      if (message.announcedTile !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.announcedTile);
      if (message.tileImage !== "")
        writer.tag(3, WireType2.LengthDelimited).string(message.tileImage);
      if (message.turnNumber !== 0)
        writer.tag(4, WireType2.Varint).int32(message.turnNumber);
      for (let i = 0; i < message.waitingForPlayers.length; i++)
        writer.tag(5, WireType2.LengthDelimited).string(message.waitingForPlayers[i]);
      if (message.gameState !== "")
        writer.tag(6, WireType2.LengthDelimited).string(message.gameState);
      if (message.error)
        Error2.internalBinaryWrite(message.error, writer.tag(7, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var StartTurnResponse = new StartTurnResponse$Type();
  var GetGameStateRequest$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GetGameStateRequest", [
        {
          no: 1,
          name: "session_id",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.sessionId = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* string session_id */
          1:
            message.sessionId = reader.string();
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.sessionId !== "")
        writer.tag(1, WireType2.LengthDelimited).string(message.sessionId);
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GetGameStateRequest = new GetGameStateRequest$Type();
  var GetGameStateResponse$Type = class extends MessageType {
    constructor() {
      super("takeiteasygame.v1.GetGameStateResponse", [
        {
          no: 1,
          name: "success",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        },
        {
          no: 2,
          name: "game_state",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 3,
          name: "current_tile",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 4,
          name: "current_tile_image",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 5,
          name: "current_turn",
          kind: "scalar",
          T: 5
          /*ScalarType.INT32*/
        },
        {
          no: 6,
          name: "waiting_for_players",
          kind: "scalar",
          repeat: 2,
          T: 9
          /*ScalarType.STRING*/
        },
        {
          no: 7,
          name: "is_game_finished",
          kind: "scalar",
          T: 8
          /*ScalarType.BOOL*/
        },
        {
          no: 8,
          name: "final_scores",
          kind: "scalar",
          T: 9
          /*ScalarType.STRING*/
        },
        { no: 9, name: "error", kind: "message", T: () => Error2 }
      ]);
    }
    create(value) {
      const message = globalThis.Object.create(this.messagePrototype);
      message.success = false;
      message.gameState = "";
      message.currentTile = "";
      message.currentTileImage = "";
      message.currentTurn = 0;
      message.waitingForPlayers = [];
      message.isGameFinished = false;
      message.finalScores = "";
      if (value !== void 0)
        reflectionMergePartial(this, message, value);
      return message;
    }
    internalBinaryRead(reader, length, options, target) {
      let message = target ?? this.create(), end = reader.pos + length;
      while (reader.pos < end) {
        let [fieldNo, wireType] = reader.tag();
        switch (fieldNo) {
          case /* bool success */
          1:
            message.success = reader.bool();
            break;
          case /* string game_state */
          2:
            message.gameState = reader.string();
            break;
          case /* string current_tile */
          3:
            message.currentTile = reader.string();
            break;
          case /* string current_tile_image */
          4:
            message.currentTileImage = reader.string();
            break;
          case /* int32 current_turn */
          5:
            message.currentTurn = reader.int32();
            break;
          case /* repeated string waiting_for_players */
          6:
            message.waitingForPlayers.push(reader.string());
            break;
          case /* bool is_game_finished */
          7:
            message.isGameFinished = reader.bool();
            break;
          case /* string final_scores */
          8:
            message.finalScores = reader.string();
            break;
          case /* takeiteasygame.v1.Error error */
          9:
            message.error = Error2.internalBinaryRead(reader, reader.uint32(), options, message.error);
            break;
          default:
            let u = options.readUnknownField;
            if (u === "throw")
              throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
            let d = reader.skip(wireType);
            if (u !== false)
              (u === true ? UnknownFieldHandler2.onRead : u)(this.typeName, message, fieldNo, wireType, d);
        }
      }
      return message;
    }
    internalBinaryWrite(message, writer, options) {
      if (message.success !== false)
        writer.tag(1, WireType2.Varint).bool(message.success);
      if (message.gameState !== "")
        writer.tag(2, WireType2.LengthDelimited).string(message.gameState);
      if (message.currentTile !== "")
        writer.tag(3, WireType2.LengthDelimited).string(message.currentTile);
      if (message.currentTileImage !== "")
        writer.tag(4, WireType2.LengthDelimited).string(message.currentTileImage);
      if (message.currentTurn !== 0)
        writer.tag(5, WireType2.Varint).int32(message.currentTurn);
      for (let i = 0; i < message.waitingForPlayers.length; i++)
        writer.tag(6, WireType2.LengthDelimited).string(message.waitingForPlayers[i]);
      if (message.isGameFinished !== false)
        writer.tag(7, WireType2.Varint).bool(message.isGameFinished);
      if (message.finalScores !== "")
        writer.tag(8, WireType2.LengthDelimited).string(message.finalScores);
      if (message.error)
        Error2.internalBinaryWrite(message.error, writer.tag(9, WireType2.LengthDelimited).fork(), options).join();
      let u = options.writeUnknownFields;
      if (u !== false)
        (u == true ? UnknownFieldHandler2.onWrite : u)(this.typeName, message, writer);
      return writer;
    }
  };
  var GetGameStateResponse = new GetGameStateResponse$Type();
  var GameService = new ServiceType("takeiteasygame.v1.GameService", [
    { name: "MakeMove", options: {}, I: MakeMoveRequest, O: MakeMoveResponse },
    { name: "GetAvailableMoves", options: {}, I: GetAvailableMovesRequest, O: GetAvailableMovesResponse },
    { name: "StartTurn", options: {}, I: StartTurnRequest, O: StartTurnResponse },
    { name: "GetGameState", options: {}, I: GetGameStateRequest, O: GetGameStateResponse }
  ]);

  // ../frontend/src/generated/game_service.client.ts
  var GameServiceClient = class {
    constructor(_transport) {
      this._transport = _transport;
    }
    typeName = GameService.typeName;
    methods = GameService.methods;
    options = GameService.options;
    /**
     * Faire un mouvement
     *
     * @generated from protobuf rpc: MakeMove
     */
    makeMove(input, options) {
      const method = this.methods[0], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
    /**
     * Obtenir les mouvements disponibles
     *
     * @generated from protobuf rpc: GetAvailableMoves
     */
    getAvailableMoves(input, options) {
      const method = this.methods[1], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
    /**
     * 🎲 NOUVELLES MÉTHODES GAMEPLAY
     *
     * @generated from protobuf rpc: StartTurn
     */
    startTurn(input, options) {
      const method = this.methods[2], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
    /**
     * @generated from protobuf rpc: GetGameState
     */
    getGameState(input, options) {
      const method = this.methods[3], opt = this._transport.mergeOptions(options);
      return stackIntercept("unary", this._transport, method, opt, input);
    }
  };

  // src/grpc-client.ts
  var GRPC_WEB_URL = "http://localhost:50052";
  var GrpcClient = class {
    sessionClient;
    gameClient;
    constructor() {
      const transport = new GrpcWebFetchTransport({
        baseUrl: GRPC_WEB_URL,
        fetchInit: { mode: "cors", credentials: "omit" },
        format: "binary",
        timeout: 1e4,
        meta: {
          "content-type": "application/grpc-web+proto",
          "accept": "application/grpc-web+proto"
        }
      });
      this.sessionClient = new SessionServiceClient(transport);
      this.gameClient = new GameServiceClient(transport);
    }
    async createSession(playerName, gameMode) {
      try {
        const request = {
          playerName,
          maxPlayers: 4,
          gameMode
        };
        const { response } = await this.sessionClient.createSession(request);
        if (response.result.oneofKind === "success") {
          const success = response.result.success;
          return {
            success: true,
            sessionCode: success.sessionCode,
            sessionId: success.sessionId,
            playerId: success.playerId,
            sessionState: success.player ? {
              sessionId: success.sessionId,
              players: [success.player],
              state: 0
            } : void 0
          };
        } else if (response.result.oneofKind === "error") {
          return { success: false, error: response.result.error.message };
        }
        return { success: false, error: "R\xE9ponse invalide" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
    async joinSession(sessionCode, playerName) {
      try {
        const request = {
          sessionCode: sessionCode.toUpperCase(),
          playerName
        };
        const { response } = await this.sessionClient.joinSession(request);
        if (response.result.oneofKind === "success") {
          const success = response.result.success;
          return {
            success: true,
            sessionCode,
            sessionId: success.sessionId,
            playerId: success.playerId,
            sessionState: success.gameState
          };
        } else if (response.result.oneofKind === "error") {
          return { success: false, error: response.result.error.message };
        }
        return { success: false, error: "R\xE9ponse invalide" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
    async setReady(sessionId, playerId) {
      try {
        const request = { sessionId, playerId, ready: true };
        const { response } = await this.sessionClient.setReady(request);
        if (response.success) {
          return { success: true, gameStarted: response.gameStarted };
        } else if (response.error) {
          return { success: false, error: response.error.message };
        }
        return { success: false, error: "\xC9chec" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
    async getSessionState(sessionId) {
      try {
        const request = { sessionId };
        const { response } = await this.sessionClient.getSessionState(request);
        if (response.gameState) {
          return { success: true, sessionState: response.gameState };
        } else if (response.error) {
          return { success: false, error: response.error.message };
        }
        return { success: false, error: "Aucun \xE9tat" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
    async startTurn(sessionId) {
      try {
        const request = { sessionId };
        const { response } = await this.gameClient.startTurn(request);
        if (response.success) {
          return {
            success: true,
            announcedTile: response.announcedTile,
            tileImage: response.tileImage,
            turnNumber: response.turnNumber,
            waitingForPlayers: response.waitingForPlayers,
            gameState: response.gameState
          };
        } else if (response.error) {
          return { success: false, error: response.error.message };
        }
        return { success: false, error: "\xC9chec" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
    async makeMove(sessionId, playerId, position) {
      try {
        const request = {
          sessionId,
          playerId,
          moveData: `{"position":${position}}`,
          timestamp: BigInt(Date.now())
        };
        const { response } = await this.gameClient.makeMove(request);
        if (response.result.oneofKind === "success") {
          const success = response.result.success;
          return {
            success: true,
            pointsEarned: success.pointsEarned,
            mctsResponse: success.mctsResponse,
            isGameOver: success.isGameOver,
            newGameState: success.newGameState || {}
          };
        } else if (response.result.oneofKind === "error") {
          return { success: false, error: response.result.error.message };
        }
        return { success: false, error: "\xC9chec" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
    async getGameState(sessionId) {
      try {
        const request = { sessionId };
        const { response } = await this.gameClient.getGameState(request);
        if (response.success) {
          return {
            success: true,
            gameState: response.gameState,
            currentTile: response.currentTile,
            currentTileImage: response.currentTileImage,
            currentTurn: response.currentTurn,
            isGameFinished: response.isGameFinished,
            finalScores: response.finalScores
          };
        } else if (response.error) {
          return { success: false, error: response.error.message };
        }
        return { success: false, error: "\xC9chec" };
      } catch (error) {
        return { success: false, error: error.message || "Erreur r\xE9seau" };
      }
    }
  };
  window.grpcClient = new GrpcClient();
})();
