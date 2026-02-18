(function(scope){
'use strict';

function F(arity, fun, wrapper) {
  wrapper.a = arity;
  wrapper.f = fun;
  return wrapper;
}

function F2(fun) {
  return F(2, fun, function(a) { return function(b) { return fun(a,b); }; })
}
function F3(fun) {
  return F(3, fun, function(a) {
    return function(b) { return function(c) { return fun(a, b, c); }; };
  });
}
function F4(fun) {
  return F(4, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return fun(a, b, c, d); }; }; };
  });
}
function F5(fun) {
  return F(5, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return fun(a, b, c, d, e); }; }; }; };
  });
}
function F6(fun) {
  return F(6, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return fun(a, b, c, d, e, f); }; }; }; }; };
  });
}
function F7(fun) {
  return F(7, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return function(g) { return fun(a, b, c, d, e, f, g); }; }; }; }; }; };
  });
}
function F8(fun) {
  return F(8, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return function(g) { return function(h) {
    return fun(a, b, c, d, e, f, g, h); }; }; }; }; }; }; };
  });
}
function F9(fun) {
  return F(9, fun, function(a) { return function(b) { return function(c) {
    return function(d) { return function(e) { return function(f) {
    return function(g) { return function(h) { return function(i) {
    return fun(a, b, c, d, e, f, g, h, i); }; }; }; }; }; }; }; };
  });
}

function A2(fun, a, b) {
  return fun.a === 2 ? fun.f(a, b) : fun(a)(b);
}
function A3(fun, a, b, c) {
  return fun.a === 3 ? fun.f(a, b, c) : fun(a)(b)(c);
}
function A4(fun, a, b, c, d) {
  return fun.a === 4 ? fun.f(a, b, c, d) : fun(a)(b)(c)(d);
}
function A5(fun, a, b, c, d, e) {
  return fun.a === 5 ? fun.f(a, b, c, d, e) : fun(a)(b)(c)(d)(e);
}
function A6(fun, a, b, c, d, e, f) {
  return fun.a === 6 ? fun.f(a, b, c, d, e, f) : fun(a)(b)(c)(d)(e)(f);
}
function A7(fun, a, b, c, d, e, f, g) {
  return fun.a === 7 ? fun.f(a, b, c, d, e, f, g) : fun(a)(b)(c)(d)(e)(f)(g);
}
function A8(fun, a, b, c, d, e, f, g, h) {
  return fun.a === 8 ? fun.f(a, b, c, d, e, f, g, h) : fun(a)(b)(c)(d)(e)(f)(g)(h);
}
function A9(fun, a, b, c, d, e, f, g, h, i) {
  return fun.a === 9 ? fun.f(a, b, c, d, e, f, g, h, i) : fun(a)(b)(c)(d)(e)(f)(g)(h)(i);
}

console.warn('Compiled in DEV mode. Follow the advice at https://elm-lang.org/0.19.1/optimize for better performance and smaller assets.');


// EQUALITY

function _Utils_eq(x, y)
{
	for (
		var pair, stack = [], isEqual = _Utils_eqHelp(x, y, 0, stack);
		isEqual && (pair = stack.pop());
		isEqual = _Utils_eqHelp(pair.a, pair.b, 0, stack)
		)
	{}

	return isEqual;
}

function _Utils_eqHelp(x, y, depth, stack)
{
	if (x === y)
	{
		return true;
	}

	if (typeof x !== 'object' || x === null || y === null)
	{
		typeof x === 'function' && _Debug_crash(5);
		return false;
	}

	if (depth > 100)
	{
		stack.push(_Utils_Tuple2(x,y));
		return true;
	}

	/**/
	if (x.$ === 'Set_elm_builtin')
	{
		x = $elm$core$Set$toList(x);
		y = $elm$core$Set$toList(y);
	}
	if (x.$ === 'RBNode_elm_builtin' || x.$ === 'RBEmpty_elm_builtin')
	{
		x = $elm$core$Dict$toList(x);
		y = $elm$core$Dict$toList(y);
	}
	//*/

	/**_UNUSED/
	if (x.$ < 0)
	{
		x = $elm$core$Dict$toList(x);
		y = $elm$core$Dict$toList(y);
	}
	//*/

	for (var key in x)
	{
		if (!_Utils_eqHelp(x[key], y[key], depth + 1, stack))
		{
			return false;
		}
	}
	return true;
}

var _Utils_equal = F2(_Utils_eq);
var _Utils_notEqual = F2(function(a, b) { return !_Utils_eq(a,b); });



// COMPARISONS

// Code in Generate/JavaScript.hs, Basics.js, and List.js depends on
// the particular integer values assigned to LT, EQ, and GT.

function _Utils_cmp(x, y, ord)
{
	if (typeof x !== 'object')
	{
		return x === y ? /*EQ*/ 0 : x < y ? /*LT*/ -1 : /*GT*/ 1;
	}

	/**/
	if (x instanceof String)
	{
		var a = x.valueOf();
		var b = y.valueOf();
		return a === b ? 0 : a < b ? -1 : 1;
	}
	//*/

	/**_UNUSED/
	if (typeof x.$ === 'undefined')
	//*/
	/**/
	if (x.$[0] === '#')
	//*/
	{
		return (ord = _Utils_cmp(x.a, y.a))
			? ord
			: (ord = _Utils_cmp(x.b, y.b))
				? ord
				: _Utils_cmp(x.c, y.c);
	}

	// traverse conses until end of a list or a mismatch
	for (; x.b && y.b && !(ord = _Utils_cmp(x.a, y.a)); x = x.b, y = y.b) {} // WHILE_CONSES
	return ord || (x.b ? /*GT*/ 1 : y.b ? /*LT*/ -1 : /*EQ*/ 0);
}

var _Utils_lt = F2(function(a, b) { return _Utils_cmp(a, b) < 0; });
var _Utils_le = F2(function(a, b) { return _Utils_cmp(a, b) < 1; });
var _Utils_gt = F2(function(a, b) { return _Utils_cmp(a, b) > 0; });
var _Utils_ge = F2(function(a, b) { return _Utils_cmp(a, b) >= 0; });

var _Utils_compare = F2(function(x, y)
{
	var n = _Utils_cmp(x, y);
	return n < 0 ? $elm$core$Basics$LT : n ? $elm$core$Basics$GT : $elm$core$Basics$EQ;
});


// COMMON VALUES

var _Utils_Tuple0_UNUSED = 0;
var _Utils_Tuple0 = { $: '#0' };

function _Utils_Tuple2_UNUSED(a, b) { return { a: a, b: b }; }
function _Utils_Tuple2(a, b) { return { $: '#2', a: a, b: b }; }

function _Utils_Tuple3_UNUSED(a, b, c) { return { a: a, b: b, c: c }; }
function _Utils_Tuple3(a, b, c) { return { $: '#3', a: a, b: b, c: c }; }

function _Utils_chr_UNUSED(c) { return c; }
function _Utils_chr(c) { return new String(c); }


// RECORDS

function _Utils_update(oldRecord, updatedFields)
{
	var newRecord = {};

	for (var key in oldRecord)
	{
		newRecord[key] = oldRecord[key];
	}

	for (var key in updatedFields)
	{
		newRecord[key] = updatedFields[key];
	}

	return newRecord;
}


// APPEND

var _Utils_append = F2(_Utils_ap);

function _Utils_ap(xs, ys)
{
	// append Strings
	if (typeof xs === 'string')
	{
		return xs + ys;
	}

	// append Lists
	if (!xs.b)
	{
		return ys;
	}
	var root = _List_Cons(xs.a, ys);
	xs = xs.b
	for (var curr = root; xs.b; xs = xs.b) // WHILE_CONS
	{
		curr = curr.b = _List_Cons(xs.a, ys);
	}
	return root;
}



var _List_Nil_UNUSED = { $: 0 };
var _List_Nil = { $: '[]' };

function _List_Cons_UNUSED(hd, tl) { return { $: 1, a: hd, b: tl }; }
function _List_Cons(hd, tl) { return { $: '::', a: hd, b: tl }; }


var _List_cons = F2(_List_Cons);

function _List_fromArray(arr)
{
	var out = _List_Nil;
	for (var i = arr.length; i--; )
	{
		out = _List_Cons(arr[i], out);
	}
	return out;
}

function _List_toArray(xs)
{
	for (var out = []; xs.b; xs = xs.b) // WHILE_CONS
	{
		out.push(xs.a);
	}
	return out;
}

var _List_map2 = F3(function(f, xs, ys)
{
	for (var arr = []; xs.b && ys.b; xs = xs.b, ys = ys.b) // WHILE_CONSES
	{
		arr.push(A2(f, xs.a, ys.a));
	}
	return _List_fromArray(arr);
});

var _List_map3 = F4(function(f, xs, ys, zs)
{
	for (var arr = []; xs.b && ys.b && zs.b; xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A3(f, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_map4 = F5(function(f, ws, xs, ys, zs)
{
	for (var arr = []; ws.b && xs.b && ys.b && zs.b; ws = ws.b, xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A4(f, ws.a, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_map5 = F6(function(f, vs, ws, xs, ys, zs)
{
	for (var arr = []; vs.b && ws.b && xs.b && ys.b && zs.b; vs = vs.b, ws = ws.b, xs = xs.b, ys = ys.b, zs = zs.b) // WHILE_CONSES
	{
		arr.push(A5(f, vs.a, ws.a, xs.a, ys.a, zs.a));
	}
	return _List_fromArray(arr);
});

var _List_sortBy = F2(function(f, xs)
{
	return _List_fromArray(_List_toArray(xs).sort(function(a, b) {
		return _Utils_cmp(f(a), f(b));
	}));
});

var _List_sortWith = F2(function(f, xs)
{
	return _List_fromArray(_List_toArray(xs).sort(function(a, b) {
		var ord = A2(f, a, b);
		return ord === $elm$core$Basics$EQ ? 0 : ord === $elm$core$Basics$LT ? -1 : 1;
	}));
});



var _JsArray_empty = [];

function _JsArray_singleton(value)
{
    return [value];
}

function _JsArray_length(array)
{
    return array.length;
}

var _JsArray_initialize = F3(function(size, offset, func)
{
    var result = new Array(size);

    for (var i = 0; i < size; i++)
    {
        result[i] = func(offset + i);
    }

    return result;
});

var _JsArray_initializeFromList = F2(function (max, ls)
{
    var result = new Array(max);

    for (var i = 0; i < max && ls.b; i++)
    {
        result[i] = ls.a;
        ls = ls.b;
    }

    result.length = i;
    return _Utils_Tuple2(result, ls);
});

var _JsArray_unsafeGet = F2(function(index, array)
{
    return array[index];
});

var _JsArray_unsafeSet = F3(function(index, value, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = array[i];
    }

    result[index] = value;
    return result;
});

var _JsArray_push = F2(function(value, array)
{
    var length = array.length;
    var result = new Array(length + 1);

    for (var i = 0; i < length; i++)
    {
        result[i] = array[i];
    }

    result[length] = value;
    return result;
});

var _JsArray_foldl = F3(function(func, acc, array)
{
    var length = array.length;

    for (var i = 0; i < length; i++)
    {
        acc = A2(func, array[i], acc);
    }

    return acc;
});

var _JsArray_foldr = F3(function(func, acc, array)
{
    for (var i = array.length - 1; i >= 0; i--)
    {
        acc = A2(func, array[i], acc);
    }

    return acc;
});

var _JsArray_map = F2(function(func, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = func(array[i]);
    }

    return result;
});

var _JsArray_indexedMap = F3(function(func, offset, array)
{
    var length = array.length;
    var result = new Array(length);

    for (var i = 0; i < length; i++)
    {
        result[i] = A2(func, offset + i, array[i]);
    }

    return result;
});

var _JsArray_slice = F3(function(from, to, array)
{
    return array.slice(from, to);
});

var _JsArray_appendN = F3(function(n, dest, source)
{
    var destLen = dest.length;
    var itemsToCopy = n - destLen;

    if (itemsToCopy > source.length)
    {
        itemsToCopy = source.length;
    }

    var size = destLen + itemsToCopy;
    var result = new Array(size);

    for (var i = 0; i < destLen; i++)
    {
        result[i] = dest[i];
    }

    for (var i = 0; i < itemsToCopy; i++)
    {
        result[i + destLen] = source[i];
    }

    return result;
});



// LOG

var _Debug_log_UNUSED = F2(function(tag, value)
{
	return value;
});

var _Debug_log = F2(function(tag, value)
{
	console.log(tag + ': ' + _Debug_toString(value));
	return value;
});


// TODOS

function _Debug_todo(moduleName, region)
{
	return function(message) {
		_Debug_crash(8, moduleName, region, message);
	};
}

function _Debug_todoCase(moduleName, region, value)
{
	return function(message) {
		_Debug_crash(9, moduleName, region, value, message);
	};
}


// TO STRING

function _Debug_toString_UNUSED(value)
{
	return '<internals>';
}

function _Debug_toString(value)
{
	return _Debug_toAnsiString(false, value);
}

function _Debug_toAnsiString(ansi, value)
{
	if (typeof value === 'function')
	{
		return _Debug_internalColor(ansi, '<function>');
	}

	if (typeof value === 'boolean')
	{
		return _Debug_ctorColor(ansi, value ? 'True' : 'False');
	}

	if (typeof value === 'number')
	{
		return _Debug_numberColor(ansi, value + '');
	}

	if (value instanceof String)
	{
		return _Debug_charColor(ansi, "'" + _Debug_addSlashes(value, true) + "'");
	}

	if (typeof value === 'string')
	{
		return _Debug_stringColor(ansi, '"' + _Debug_addSlashes(value, false) + '"');
	}

	if (typeof value === 'object' && '$' in value)
	{
		var tag = value.$;

		if (typeof tag === 'number')
		{
			return _Debug_internalColor(ansi, '<internals>');
		}

		if (tag[0] === '#')
		{
			var output = [];
			for (var k in value)
			{
				if (k === '$') continue;
				output.push(_Debug_toAnsiString(ansi, value[k]));
			}
			return '(' + output.join(',') + ')';
		}

		if (tag === 'Set_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Set')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Set$toList(value));
		}

		if (tag === 'RBNode_elm_builtin' || tag === 'RBEmpty_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Dict')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Dict$toList(value));
		}

		if (tag === 'Array_elm_builtin')
		{
			return _Debug_ctorColor(ansi, 'Array')
				+ _Debug_fadeColor(ansi, '.fromList') + ' '
				+ _Debug_toAnsiString(ansi, $elm$core$Array$toList(value));
		}

		if (tag === '::' || tag === '[]')
		{
			var output = '[';

			value.b && (output += _Debug_toAnsiString(ansi, value.a), value = value.b)

			for (; value.b; value = value.b) // WHILE_CONS
			{
				output += ',' + _Debug_toAnsiString(ansi, value.a);
			}
			return output + ']';
		}

		var output = '';
		for (var i in value)
		{
			if (i === '$') continue;
			var str = _Debug_toAnsiString(ansi, value[i]);
			var c0 = str[0];
			var parenless = c0 === '{' || c0 === '(' || c0 === '[' || c0 === '<' || c0 === '"' || str.indexOf(' ') < 0;
			output += ' ' + (parenless ? str : '(' + str + ')');
		}
		return _Debug_ctorColor(ansi, tag) + output;
	}

	if (typeof DataView === 'function' && value instanceof DataView)
	{
		return _Debug_stringColor(ansi, '<' + value.byteLength + ' bytes>');
	}

	if (typeof File !== 'undefined' && value instanceof File)
	{
		return _Debug_internalColor(ansi, '<' + value.name + '>');
	}

	if (typeof value === 'object')
	{
		var output = [];
		for (var key in value)
		{
			var field = key[0] === '_' ? key.slice(1) : key;
			output.push(_Debug_fadeColor(ansi, field) + ' = ' + _Debug_toAnsiString(ansi, value[key]));
		}
		if (output.length === 0)
		{
			return '{}';
		}
		return '{ ' + output.join(', ') + ' }';
	}

	return _Debug_internalColor(ansi, '<internals>');
}

function _Debug_addSlashes(str, isChar)
{
	var s = str
		.replace(/\\/g, '\\\\')
		.replace(/\n/g, '\\n')
		.replace(/\t/g, '\\t')
		.replace(/\r/g, '\\r')
		.replace(/\v/g, '\\v')
		.replace(/\0/g, '\\0');

	if (isChar)
	{
		return s.replace(/\'/g, '\\\'');
	}
	else
	{
		return s.replace(/\"/g, '\\"');
	}
}

function _Debug_ctorColor(ansi, string)
{
	return ansi ? '\x1b[96m' + string + '\x1b[0m' : string;
}

function _Debug_numberColor(ansi, string)
{
	return ansi ? '\x1b[95m' + string + '\x1b[0m' : string;
}

function _Debug_stringColor(ansi, string)
{
	return ansi ? '\x1b[93m' + string + '\x1b[0m' : string;
}

function _Debug_charColor(ansi, string)
{
	return ansi ? '\x1b[92m' + string + '\x1b[0m' : string;
}

function _Debug_fadeColor(ansi, string)
{
	return ansi ? '\x1b[37m' + string + '\x1b[0m' : string;
}

function _Debug_internalColor(ansi, string)
{
	return ansi ? '\x1b[36m' + string + '\x1b[0m' : string;
}

function _Debug_toHexDigit(n)
{
	return String.fromCharCode(n < 10 ? 48 + n : 55 + n);
}


// CRASH


function _Debug_crash_UNUSED(identifier)
{
	throw new Error('https://github.com/elm/core/blob/1.0.0/hints/' + identifier + '.md');
}


function _Debug_crash(identifier, fact1, fact2, fact3, fact4)
{
	switch(identifier)
	{
		case 0:
			throw new Error('What node should I take over? In JavaScript I need something like:\n\n    Elm.Main.init({\n        node: document.getElementById("elm-node")\n    })\n\nYou need to do this with any Browser.sandbox or Browser.element program.');

		case 1:
			throw new Error('Browser.application programs cannot handle URLs like this:\n\n    ' + document.location.href + '\n\nWhat is the root? The root of your file system? Try looking at this program with `elm reactor` or some other server.');

		case 2:
			var jsonErrorString = fact1;
			throw new Error('Problem with the flags given to your Elm program on initialization.\n\n' + jsonErrorString);

		case 3:
			var portName = fact1;
			throw new Error('There can only be one port named `' + portName + '`, but your program has multiple.');

		case 4:
			var portName = fact1;
			var problem = fact2;
			throw new Error('Trying to send an unexpected type of value through port `' + portName + '`:\n' + problem);

		case 5:
			throw new Error('Trying to use `(==)` on functions.\nThere is no way to know if functions are "the same" in the Elm sense.\nRead more about this at https://package.elm-lang.org/packages/elm/core/latest/Basics#== which describes why it is this way and what the better version will look like.');

		case 6:
			var moduleName = fact1;
			throw new Error('Your page is loading multiple Elm scripts with a module named ' + moduleName + '. Maybe a duplicate script is getting loaded accidentally? If not, rename one of them so I know which is which!');

		case 8:
			var moduleName = fact1;
			var region = fact2;
			var message = fact3;
			throw new Error('TODO in module `' + moduleName + '` ' + _Debug_regionToString(region) + '\n\n' + message);

		case 9:
			var moduleName = fact1;
			var region = fact2;
			var value = fact3;
			var message = fact4;
			throw new Error(
				'TODO in module `' + moduleName + '` from the `case` expression '
				+ _Debug_regionToString(region) + '\n\nIt received the following value:\n\n    '
				+ _Debug_toString(value).replace('\n', '\n    ')
				+ '\n\nBut the branch that handles it says:\n\n    ' + message.replace('\n', '\n    ')
			);

		case 10:
			throw new Error('Bug in https://github.com/elm/virtual-dom/issues');

		case 11:
			throw new Error('Cannot perform mod 0. Division by zero error.');
	}
}

function _Debug_regionToString(region)
{
	if (region.start.line === region.end.line)
	{
		return 'on line ' + region.start.line;
	}
	return 'on lines ' + region.start.line + ' through ' + region.end.line;
}



// MATH

var _Basics_add = F2(function(a, b) { return a + b; });
var _Basics_sub = F2(function(a, b) { return a - b; });
var _Basics_mul = F2(function(a, b) { return a * b; });
var _Basics_fdiv = F2(function(a, b) { return a / b; });
var _Basics_idiv = F2(function(a, b) { return (a / b) | 0; });
var _Basics_pow = F2(Math.pow);

var _Basics_remainderBy = F2(function(b, a) { return a % b; });

// https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/divmodnote-letter.pdf
var _Basics_modBy = F2(function(modulus, x)
{
	var answer = x % modulus;
	return modulus === 0
		? _Debug_crash(11)
		:
	((answer > 0 && modulus < 0) || (answer < 0 && modulus > 0))
		? answer + modulus
		: answer;
});


// TRIGONOMETRY

var _Basics_pi = Math.PI;
var _Basics_e = Math.E;
var _Basics_cos = Math.cos;
var _Basics_sin = Math.sin;
var _Basics_tan = Math.tan;
var _Basics_acos = Math.acos;
var _Basics_asin = Math.asin;
var _Basics_atan = Math.atan;
var _Basics_atan2 = F2(Math.atan2);


// MORE MATH

function _Basics_toFloat(x) { return x; }
function _Basics_truncate(n) { return n | 0; }
function _Basics_isInfinite(n) { return n === Infinity || n === -Infinity; }

var _Basics_ceiling = Math.ceil;
var _Basics_floor = Math.floor;
var _Basics_round = Math.round;
var _Basics_sqrt = Math.sqrt;
var _Basics_log = Math.log;
var _Basics_isNaN = isNaN;


// BOOLEANS

function _Basics_not(bool) { return !bool; }
var _Basics_and = F2(function(a, b) { return a && b; });
var _Basics_or  = F2(function(a, b) { return a || b; });
var _Basics_xor = F2(function(a, b) { return a !== b; });



var _String_cons = F2(function(chr, str)
{
	return chr + str;
});

function _String_uncons(string)
{
	var word = string.charCodeAt(0);
	return !isNaN(word)
		? $elm$core$Maybe$Just(
			0xD800 <= word && word <= 0xDBFF
				? _Utils_Tuple2(_Utils_chr(string[0] + string[1]), string.slice(2))
				: _Utils_Tuple2(_Utils_chr(string[0]), string.slice(1))
		)
		: $elm$core$Maybe$Nothing;
}

var _String_append = F2(function(a, b)
{
	return a + b;
});

function _String_length(str)
{
	return str.length;
}

var _String_map = F2(function(func, string)
{
	var len = string.length;
	var array = new Array(len);
	var i = 0;
	while (i < len)
	{
		var word = string.charCodeAt(i);
		if (0xD800 <= word && word <= 0xDBFF)
		{
			array[i] = func(_Utils_chr(string[i] + string[i+1]));
			i += 2;
			continue;
		}
		array[i] = func(_Utils_chr(string[i]));
		i++;
	}
	return array.join('');
});

var _String_filter = F2(function(isGood, str)
{
	var arr = [];
	var len = str.length;
	var i = 0;
	while (i < len)
	{
		var char = str[i];
		var word = str.charCodeAt(i);
		i++;
		if (0xD800 <= word && word <= 0xDBFF)
		{
			char += str[i];
			i++;
		}

		if (isGood(_Utils_chr(char)))
		{
			arr.push(char);
		}
	}
	return arr.join('');
});

function _String_reverse(str)
{
	var len = str.length;
	var arr = new Array(len);
	var i = 0;
	while (i < len)
	{
		var word = str.charCodeAt(i);
		if (0xD800 <= word && word <= 0xDBFF)
		{
			arr[len - i] = str[i + 1];
			i++;
			arr[len - i] = str[i - 1];
			i++;
		}
		else
		{
			arr[len - i] = str[i];
			i++;
		}
	}
	return arr.join('');
}

var _String_foldl = F3(function(func, state, string)
{
	var len = string.length;
	var i = 0;
	while (i < len)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		i++;
		if (0xD800 <= word && word <= 0xDBFF)
		{
			char += string[i];
			i++;
		}
		state = A2(func, _Utils_chr(char), state);
	}
	return state;
});

var _String_foldr = F3(function(func, state, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		state = A2(func, _Utils_chr(char), state);
	}
	return state;
});

var _String_split = F2(function(sep, str)
{
	return str.split(sep);
});

var _String_join = F2(function(sep, strs)
{
	return strs.join(sep);
});

var _String_slice = F3(function(start, end, str) {
	return str.slice(start, end);
});

function _String_trim(str)
{
	return str.trim();
}

function _String_trimLeft(str)
{
	return str.replace(/^\s+/, '');
}

function _String_trimRight(str)
{
	return str.replace(/\s+$/, '');
}

function _String_words(str)
{
	return _List_fromArray(str.trim().split(/\s+/g));
}

function _String_lines(str)
{
	return _List_fromArray(str.split(/\r\n|\r|\n/g));
}

function _String_toUpper(str)
{
	return str.toUpperCase();
}

function _String_toLower(str)
{
	return str.toLowerCase();
}

var _String_any = F2(function(isGood, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		if (isGood(_Utils_chr(char)))
		{
			return true;
		}
	}
	return false;
});

var _String_all = F2(function(isGood, string)
{
	var i = string.length;
	while (i--)
	{
		var char = string[i];
		var word = string.charCodeAt(i);
		if (0xDC00 <= word && word <= 0xDFFF)
		{
			i--;
			char = string[i] + char;
		}
		if (!isGood(_Utils_chr(char)))
		{
			return false;
		}
	}
	return true;
});

var _String_contains = F2(function(sub, str)
{
	return str.indexOf(sub) > -1;
});

var _String_startsWith = F2(function(sub, str)
{
	return str.indexOf(sub) === 0;
});

var _String_endsWith = F2(function(sub, str)
{
	return str.length >= sub.length &&
		str.lastIndexOf(sub) === str.length - sub.length;
});

var _String_indexes = F2(function(sub, str)
{
	var subLen = sub.length;

	if (subLen < 1)
	{
		return _List_Nil;
	}

	var i = 0;
	var is = [];

	while ((i = str.indexOf(sub, i)) > -1)
	{
		is.push(i);
		i = i + subLen;
	}

	return _List_fromArray(is);
});


// TO STRING

function _String_fromNumber(number)
{
	return number + '';
}


// INT CONVERSIONS

function _String_toInt(str)
{
	var total = 0;
	var code0 = str.charCodeAt(0);
	var start = code0 == 0x2B /* + */ || code0 == 0x2D /* - */ ? 1 : 0;

	for (var i = start; i < str.length; ++i)
	{
		var code = str.charCodeAt(i);
		if (code < 0x30 || 0x39 < code)
		{
			return $elm$core$Maybe$Nothing;
		}
		total = 10 * total + code - 0x30;
	}

	return i == start
		? $elm$core$Maybe$Nothing
		: $elm$core$Maybe$Just(code0 == 0x2D ? -total : total);
}


// FLOAT CONVERSIONS

function _String_toFloat(s)
{
	// check if it is a hex, octal, or binary number
	if (s.length === 0 || /[\sxbo]/.test(s))
	{
		return $elm$core$Maybe$Nothing;
	}
	var n = +s;
	// faster isNaN check
	return n === n ? $elm$core$Maybe$Just(n) : $elm$core$Maybe$Nothing;
}

function _String_fromList(chars)
{
	return _List_toArray(chars).join('');
}




function _Char_toCode(char)
{
	var code = char.charCodeAt(0);
	if (0xD800 <= code && code <= 0xDBFF)
	{
		return (code - 0xD800) * 0x400 + char.charCodeAt(1) - 0xDC00 + 0x10000
	}
	return code;
}

function _Char_fromCode(code)
{
	return _Utils_chr(
		(code < 0 || 0x10FFFF < code)
			? '\uFFFD'
			:
		(code <= 0xFFFF)
			? String.fromCharCode(code)
			:
		(code -= 0x10000,
			String.fromCharCode(Math.floor(code / 0x400) + 0xD800, code % 0x400 + 0xDC00)
		)
	);
}

function _Char_toUpper(char)
{
	return _Utils_chr(char.toUpperCase());
}

function _Char_toLower(char)
{
	return _Utils_chr(char.toLowerCase());
}

function _Char_toLocaleUpper(char)
{
	return _Utils_chr(char.toLocaleUpperCase());
}

function _Char_toLocaleLower(char)
{
	return _Utils_chr(char.toLocaleLowerCase());
}



/**/
function _Json_errorToString(error)
{
	return $elm$json$Json$Decode$errorToString(error);
}
//*/


// CORE DECODERS

function _Json_succeed(msg)
{
	return {
		$: 0,
		a: msg
	};
}

function _Json_fail(msg)
{
	return {
		$: 1,
		a: msg
	};
}

function _Json_decodePrim(decoder)
{
	return { $: 2, b: decoder };
}

var _Json_decodeInt = _Json_decodePrim(function(value) {
	return (typeof value !== 'number')
		? _Json_expecting('an INT', value)
		:
	(-2147483647 < value && value < 2147483647 && (value | 0) === value)
		? $elm$core$Result$Ok(value)
		:
	(isFinite(value) && !(value % 1))
		? $elm$core$Result$Ok(value)
		: _Json_expecting('an INT', value);
});

var _Json_decodeBool = _Json_decodePrim(function(value) {
	return (typeof value === 'boolean')
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a BOOL', value);
});

var _Json_decodeFloat = _Json_decodePrim(function(value) {
	return (typeof value === 'number')
		? $elm$core$Result$Ok(value)
		: _Json_expecting('a FLOAT', value);
});

var _Json_decodeValue = _Json_decodePrim(function(value) {
	return $elm$core$Result$Ok(_Json_wrap(value));
});

var _Json_decodeString = _Json_decodePrim(function(value) {
	return (typeof value === 'string')
		? $elm$core$Result$Ok(value)
		: (value instanceof String)
			? $elm$core$Result$Ok(value + '')
			: _Json_expecting('a STRING', value);
});

function _Json_decodeList(decoder) { return { $: 3, b: decoder }; }
function _Json_decodeArray(decoder) { return { $: 4, b: decoder }; }

function _Json_decodeNull(value) { return { $: 5, c: value }; }

var _Json_decodeField = F2(function(field, decoder)
{
	return {
		$: 6,
		d: field,
		b: decoder
	};
});

var _Json_decodeIndex = F2(function(index, decoder)
{
	return {
		$: 7,
		e: index,
		b: decoder
	};
});

function _Json_decodeKeyValuePairs(decoder)
{
	return {
		$: 8,
		b: decoder
	};
}

function _Json_mapMany(f, decoders)
{
	return {
		$: 9,
		f: f,
		g: decoders
	};
}

var _Json_andThen = F2(function(callback, decoder)
{
	return {
		$: 10,
		b: decoder,
		h: callback
	};
});

function _Json_oneOf(decoders)
{
	return {
		$: 11,
		g: decoders
	};
}


// DECODING OBJECTS

var _Json_map1 = F2(function(f, d1)
{
	return _Json_mapMany(f, [d1]);
});

var _Json_map2 = F3(function(f, d1, d2)
{
	return _Json_mapMany(f, [d1, d2]);
});

var _Json_map3 = F4(function(f, d1, d2, d3)
{
	return _Json_mapMany(f, [d1, d2, d3]);
});

var _Json_map4 = F5(function(f, d1, d2, d3, d4)
{
	return _Json_mapMany(f, [d1, d2, d3, d4]);
});

var _Json_map5 = F6(function(f, d1, d2, d3, d4, d5)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5]);
});

var _Json_map6 = F7(function(f, d1, d2, d3, d4, d5, d6)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6]);
});

var _Json_map7 = F8(function(f, d1, d2, d3, d4, d5, d6, d7)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6, d7]);
});

var _Json_map8 = F9(function(f, d1, d2, d3, d4, d5, d6, d7, d8)
{
	return _Json_mapMany(f, [d1, d2, d3, d4, d5, d6, d7, d8]);
});


// DECODE

var _Json_runOnString = F2(function(decoder, string)
{
	try
	{
		var value = JSON.parse(string);
		return _Json_runHelp(decoder, value);
	}
	catch (e)
	{
		return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, 'This is not valid JSON! ' + e.message, _Json_wrap(string)));
	}
});

var _Json_run = F2(function(decoder, value)
{
	return _Json_runHelp(decoder, _Json_unwrap(value));
});

function _Json_runHelp(decoder, value)
{
	switch (decoder.$)
	{
		case 2:
			return decoder.b(value);

		case 5:
			return (value === null)
				? $elm$core$Result$Ok(decoder.c)
				: _Json_expecting('null', value);

		case 3:
			if (!_Json_isArray(value))
			{
				return _Json_expecting('a LIST', value);
			}
			return _Json_runArrayDecoder(decoder.b, value, _List_fromArray);

		case 4:
			if (!_Json_isArray(value))
			{
				return _Json_expecting('an ARRAY', value);
			}
			return _Json_runArrayDecoder(decoder.b, value, _Json_toElmArray);

		case 6:
			var field = decoder.d;
			if (typeof value !== 'object' || value === null || !(field in value))
			{
				return _Json_expecting('an OBJECT with a field named `' + field + '`', value);
			}
			var result = _Json_runHelp(decoder.b, value[field]);
			return ($elm$core$Result$isOk(result)) ? result : $elm$core$Result$Err(A2($elm$json$Json$Decode$Field, field, result.a));

		case 7:
			var index = decoder.e;
			if (!_Json_isArray(value))
			{
				return _Json_expecting('an ARRAY', value);
			}
			if (index >= value.length)
			{
				return _Json_expecting('a LONGER array. Need index ' + index + ' but only see ' + value.length + ' entries', value);
			}
			var result = _Json_runHelp(decoder.b, value[index]);
			return ($elm$core$Result$isOk(result)) ? result : $elm$core$Result$Err(A2($elm$json$Json$Decode$Index, index, result.a));

		case 8:
			if (typeof value !== 'object' || value === null || _Json_isArray(value))
			{
				return _Json_expecting('an OBJECT', value);
			}

			var keyValuePairs = _List_Nil;
			// TODO test perf of Object.keys and switch when support is good enough
			for (var key in value)
			{
				if (Object.prototype.hasOwnProperty.call(value, key))
				{
					var result = _Json_runHelp(decoder.b, value[key]);
					if (!$elm$core$Result$isOk(result))
					{
						return $elm$core$Result$Err(A2($elm$json$Json$Decode$Field, key, result.a));
					}
					keyValuePairs = _List_Cons(_Utils_Tuple2(key, result.a), keyValuePairs);
				}
			}
			return $elm$core$Result$Ok($elm$core$List$reverse(keyValuePairs));

		case 9:
			var answer = decoder.f;
			var decoders = decoder.g;
			for (var i = 0; i < decoders.length; i++)
			{
				var result = _Json_runHelp(decoders[i], value);
				if (!$elm$core$Result$isOk(result))
				{
					return result;
				}
				answer = answer(result.a);
			}
			return $elm$core$Result$Ok(answer);

		case 10:
			var result = _Json_runHelp(decoder.b, value);
			return (!$elm$core$Result$isOk(result))
				? result
				: _Json_runHelp(decoder.h(result.a), value);

		case 11:
			var errors = _List_Nil;
			for (var temp = decoder.g; temp.b; temp = temp.b) // WHILE_CONS
			{
				var result = _Json_runHelp(temp.a, value);
				if ($elm$core$Result$isOk(result))
				{
					return result;
				}
				errors = _List_Cons(result.a, errors);
			}
			return $elm$core$Result$Err($elm$json$Json$Decode$OneOf($elm$core$List$reverse(errors)));

		case 1:
			return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, decoder.a, _Json_wrap(value)));

		case 0:
			return $elm$core$Result$Ok(decoder.a);
	}
}

function _Json_runArrayDecoder(decoder, value, toElmValue)
{
	var len = value.length;
	var array = new Array(len);
	for (var i = 0; i < len; i++)
	{
		var result = _Json_runHelp(decoder, value[i]);
		if (!$elm$core$Result$isOk(result))
		{
			return $elm$core$Result$Err(A2($elm$json$Json$Decode$Index, i, result.a));
		}
		array[i] = result.a;
	}
	return $elm$core$Result$Ok(toElmValue(array));
}

function _Json_isArray(value)
{
	return Array.isArray(value) || (typeof FileList !== 'undefined' && value instanceof FileList);
}

function _Json_toElmArray(array)
{
	return A2($elm$core$Array$initialize, array.length, function(i) { return array[i]; });
}

function _Json_expecting(type, value)
{
	return $elm$core$Result$Err(A2($elm$json$Json$Decode$Failure, 'Expecting ' + type, _Json_wrap(value)));
}


// EQUALITY

function _Json_equality(x, y)
{
	if (x === y)
	{
		return true;
	}

	if (x.$ !== y.$)
	{
		return false;
	}

	switch (x.$)
	{
		case 0:
		case 1:
			return x.a === y.a;

		case 2:
			return x.b === y.b;

		case 5:
			return x.c === y.c;

		case 3:
		case 4:
		case 8:
			return _Json_equality(x.b, y.b);

		case 6:
			return x.d === y.d && _Json_equality(x.b, y.b);

		case 7:
			return x.e === y.e && _Json_equality(x.b, y.b);

		case 9:
			return x.f === y.f && _Json_listEquality(x.g, y.g);

		case 10:
			return x.h === y.h && _Json_equality(x.b, y.b);

		case 11:
			return _Json_listEquality(x.g, y.g);
	}
}

function _Json_listEquality(aDecoders, bDecoders)
{
	var len = aDecoders.length;
	if (len !== bDecoders.length)
	{
		return false;
	}
	for (var i = 0; i < len; i++)
	{
		if (!_Json_equality(aDecoders[i], bDecoders[i]))
		{
			return false;
		}
	}
	return true;
}


// ENCODE

var _Json_encode = F2(function(indentLevel, value)
{
	return JSON.stringify(_Json_unwrap(value), null, indentLevel) + '';
});

function _Json_wrap(value) { return { $: 0, a: value }; }
function _Json_unwrap(value) { return value.a; }

function _Json_wrap_UNUSED(value) { return value; }
function _Json_unwrap_UNUSED(value) { return value; }

function _Json_emptyArray() { return []; }
function _Json_emptyObject() { return {}; }

var _Json_addField = F3(function(key, value, object)
{
	var unwrapped = _Json_unwrap(value);
	if (!(key === 'toJSON' && typeof unwrapped === 'function'))
	{
		object[key] = unwrapped;
	}
	return object;
});

function _Json_addEntry(func)
{
	return F2(function(entry, array)
	{
		array.push(_Json_unwrap(func(entry)));
		return array;
	});
}

var _Json_encodeNull = _Json_wrap(null);



// TASKS

function _Scheduler_succeed(value)
{
	return {
		$: 0,
		a: value
	};
}

function _Scheduler_fail(error)
{
	return {
		$: 1,
		a: error
	};
}

function _Scheduler_binding(callback)
{
	return {
		$: 2,
		b: callback,
		c: null
	};
}

var _Scheduler_andThen = F2(function(callback, task)
{
	return {
		$: 3,
		b: callback,
		d: task
	};
});

var _Scheduler_onError = F2(function(callback, task)
{
	return {
		$: 4,
		b: callback,
		d: task
	};
});

function _Scheduler_receive(callback)
{
	return {
		$: 5,
		b: callback
	};
}


// PROCESSES

var _Scheduler_guid = 0;

function _Scheduler_rawSpawn(task)
{
	var proc = {
		$: 0,
		e: _Scheduler_guid++,
		f: task,
		g: null,
		h: []
	};

	_Scheduler_enqueue(proc);

	return proc;
}

function _Scheduler_spawn(task)
{
	return _Scheduler_binding(function(callback) {
		callback(_Scheduler_succeed(_Scheduler_rawSpawn(task)));
	});
}

function _Scheduler_rawSend(proc, msg)
{
	proc.h.push(msg);
	_Scheduler_enqueue(proc);
}

var _Scheduler_send = F2(function(proc, msg)
{
	return _Scheduler_binding(function(callback) {
		_Scheduler_rawSend(proc, msg);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
});

function _Scheduler_kill(proc)
{
	return _Scheduler_binding(function(callback) {
		var task = proc.f;
		if (task.$ === 2 && task.c)
		{
			task.c();
		}

		proc.f = null;

		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
}


/* STEP PROCESSES

type alias Process =
  { $ : tag
  , id : unique_id
  , root : Task
  , stack : null | { $: SUCCEED | FAIL, a: callback, b: stack }
  , mailbox : [msg]
  }

*/


var _Scheduler_working = false;
var _Scheduler_queue = [];


function _Scheduler_enqueue(proc)
{
	_Scheduler_queue.push(proc);
	if (_Scheduler_working)
	{
		return;
	}
	_Scheduler_working = true;
	while (proc = _Scheduler_queue.shift())
	{
		_Scheduler_step(proc);
	}
	_Scheduler_working = false;
}


function _Scheduler_step(proc)
{
	while (proc.f)
	{
		var rootTag = proc.f.$;
		if (rootTag === 0 || rootTag === 1)
		{
			while (proc.g && proc.g.$ !== rootTag)
			{
				proc.g = proc.g.i;
			}
			if (!proc.g)
			{
				return;
			}
			proc.f = proc.g.b(proc.f.a);
			proc.g = proc.g.i;
		}
		else if (rootTag === 2)
		{
			proc.f.c = proc.f.b(function(newRoot) {
				proc.f = newRoot;
				_Scheduler_enqueue(proc);
			});
			return;
		}
		else if (rootTag === 5)
		{
			if (proc.h.length === 0)
			{
				return;
			}
			proc.f = proc.f.b(proc.h.shift());
		}
		else // if (rootTag === 3 || rootTag === 4)
		{
			proc.g = {
				$: rootTag === 3 ? 0 : 1,
				b: proc.f.b,
				i: proc.g
			};
			proc.f = proc.f.d;
		}
	}
}



function _Process_sleep(time)
{
	return _Scheduler_binding(function(callback) {
		var id = setTimeout(function() {
			callback(_Scheduler_succeed(_Utils_Tuple0));
		}, time);

		return function() { clearTimeout(id); };
	});
}




// PROGRAMS


var _Platform_worker = F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.init,
		impl.update,
		impl.subscriptions,
		function() { return function() {} }
	);
});



// INITIALIZE A PROGRAM


function _Platform_initialize(flagDecoder, args, init, update, subscriptions, stepperBuilder)
{
	var result = A2(_Json_run, flagDecoder, _Json_wrap(args ? args['flags'] : undefined));
	$elm$core$Result$isOk(result) || _Debug_crash(2 /**/, _Json_errorToString(result.a) /**/);
	var managers = {};
	var initPair = init(result.a);
	var model = initPair.a;
	var stepper = stepperBuilder(sendToApp, model);
	var ports = _Platform_setupEffects(managers, sendToApp);

	function sendToApp(msg, viewMetadata)
	{
		var pair = A2(update, msg, model);
		stepper(model = pair.a, viewMetadata);
		_Platform_enqueueEffects(managers, pair.b, subscriptions(model));
	}

	_Platform_enqueueEffects(managers, initPair.b, subscriptions(model));

	return ports ? { ports: ports } : {};
}



// TRACK PRELOADS
//
// This is used by code in elm/browser and elm/http
// to register any HTTP requests that are triggered by init.
//


var _Platform_preload;


function _Platform_registerPreload(url)
{
	_Platform_preload.add(url);
}



// EFFECT MANAGERS


var _Platform_effectManagers = {};


function _Platform_setupEffects(managers, sendToApp)
{
	var ports;

	// setup all necessary effect managers
	for (var key in _Platform_effectManagers)
	{
		var manager = _Platform_effectManagers[key];

		if (manager.a)
		{
			ports = ports || {};
			ports[key] = manager.a(key, sendToApp);
		}

		managers[key] = _Platform_instantiateManager(manager, sendToApp);
	}

	return ports;
}


function _Platform_createManager(init, onEffects, onSelfMsg, cmdMap, subMap)
{
	return {
		b: init,
		c: onEffects,
		d: onSelfMsg,
		e: cmdMap,
		f: subMap
	};
}


function _Platform_instantiateManager(info, sendToApp)
{
	var router = {
		g: sendToApp,
		h: undefined
	};

	var onEffects = info.c;
	var onSelfMsg = info.d;
	var cmdMap = info.e;
	var subMap = info.f;

	function loop(state)
	{
		return A2(_Scheduler_andThen, loop, _Scheduler_receive(function(msg)
		{
			var value = msg.a;

			if (msg.$ === 0)
			{
				return A3(onSelfMsg, router, value, state);
			}

			return cmdMap && subMap
				? A4(onEffects, router, value.i, value.j, state)
				: A3(onEffects, router, cmdMap ? value.i : value.j, state);
		}));
	}

	return router.h = _Scheduler_rawSpawn(A2(_Scheduler_andThen, loop, info.b));
}



// ROUTING


var _Platform_sendToApp = F2(function(router, msg)
{
	return _Scheduler_binding(function(callback)
	{
		router.g(msg);
		callback(_Scheduler_succeed(_Utils_Tuple0));
	});
});


var _Platform_sendToSelf = F2(function(router, msg)
{
	return A2(_Scheduler_send, router.h, {
		$: 0,
		a: msg
	});
});



// BAGS


function _Platform_leaf(home)
{
	return function(value)
	{
		return {
			$: 1,
			k: home,
			l: value
		};
	};
}


function _Platform_batch(list)
{
	return {
		$: 2,
		m: list
	};
}


var _Platform_map = F2(function(tagger, bag)
{
	return {
		$: 3,
		n: tagger,
		o: bag
	}
});



// PIPE BAGS INTO EFFECT MANAGERS
//
// Effects must be queued!
//
// Say your init contains a synchronous command, like Time.now or Time.here
//
//   - This will produce a batch of effects (FX_1)
//   - The synchronous task triggers the subsequent `update` call
//   - This will produce a batch of effects (FX_2)
//
// If we just start dispatching FX_2, subscriptions from FX_2 can be processed
// before subscriptions from FX_1. No good! Earlier versions of this code had
// this problem, leading to these reports:
//
//   https://github.com/elm/core/issues/980
//   https://github.com/elm/core/pull/981
//   https://github.com/elm/compiler/issues/1776
//
// The queue is necessary to avoid ordering issues for synchronous commands.


// Why use true/false here? Why not just check the length of the queue?
// The goal is to detect "are we currently dispatching effects?" If we
// are, we need to bail and let the ongoing while loop handle things.
//
// Now say the queue has 1 element. When we dequeue the final element,
// the queue will be empty, but we are still actively dispatching effects.
// So you could get queue jumping in a really tricky category of cases.
//
var _Platform_effectsQueue = [];
var _Platform_effectsActive = false;


function _Platform_enqueueEffects(managers, cmdBag, subBag)
{
	_Platform_effectsQueue.push({ p: managers, q: cmdBag, r: subBag });

	if (_Platform_effectsActive) return;

	_Platform_effectsActive = true;
	for (var fx; fx = _Platform_effectsQueue.shift(); )
	{
		_Platform_dispatchEffects(fx.p, fx.q, fx.r);
	}
	_Platform_effectsActive = false;
}


function _Platform_dispatchEffects(managers, cmdBag, subBag)
{
	var effectsDict = {};
	_Platform_gatherEffects(true, cmdBag, effectsDict, null);
	_Platform_gatherEffects(false, subBag, effectsDict, null);

	for (var home in managers)
	{
		_Scheduler_rawSend(managers[home], {
			$: 'fx',
			a: effectsDict[home] || { i: _List_Nil, j: _List_Nil }
		});
	}
}


function _Platform_gatherEffects(isCmd, bag, effectsDict, taggers)
{
	switch (bag.$)
	{
		case 1:
			var home = bag.k;
			var effect = _Platform_toEffect(isCmd, home, taggers, bag.l);
			effectsDict[home] = _Platform_insert(isCmd, effect, effectsDict[home]);
			return;

		case 2:
			for (var list = bag.m; list.b; list = list.b) // WHILE_CONS
			{
				_Platform_gatherEffects(isCmd, list.a, effectsDict, taggers);
			}
			return;

		case 3:
			_Platform_gatherEffects(isCmd, bag.o, effectsDict, {
				s: bag.n,
				t: taggers
			});
			return;
	}
}


function _Platform_toEffect(isCmd, home, taggers, value)
{
	function applyTaggers(x)
	{
		for (var temp = taggers; temp; temp = temp.t)
		{
			x = temp.s(x);
		}
		return x;
	}

	var map = isCmd
		? _Platform_effectManagers[home].e
		: _Platform_effectManagers[home].f;

	return A2(map, applyTaggers, value)
}


function _Platform_insert(isCmd, newEffect, effects)
{
	effects = effects || { i: _List_Nil, j: _List_Nil };

	isCmd
		? (effects.i = _List_Cons(newEffect, effects.i))
		: (effects.j = _List_Cons(newEffect, effects.j));

	return effects;
}



// PORTS


function _Platform_checkPortName(name)
{
	if (_Platform_effectManagers[name])
	{
		_Debug_crash(3, name)
	}
}



// OUTGOING PORTS


function _Platform_outgoingPort(name, converter)
{
	_Platform_checkPortName(name);
	_Platform_effectManagers[name] = {
		e: _Platform_outgoingPortMap,
		u: converter,
		a: _Platform_setupOutgoingPort
	};
	return _Platform_leaf(name);
}


var _Platform_outgoingPortMap = F2(function(tagger, value) { return value; });


function _Platform_setupOutgoingPort(name)
{
	var subs = [];
	var converter = _Platform_effectManagers[name].u;

	// CREATE MANAGER

	var init = _Process_sleep(0);

	_Platform_effectManagers[name].b = init;
	_Platform_effectManagers[name].c = F3(function(router, cmdList, state)
	{
		for ( ; cmdList.b; cmdList = cmdList.b) // WHILE_CONS
		{
			// grab a separate reference to subs in case unsubscribe is called
			var currentSubs = subs;
			var value = _Json_unwrap(converter(cmdList.a));
			for (var i = 0; i < currentSubs.length; i++)
			{
				currentSubs[i](value);
			}
		}
		return init;
	});

	// PUBLIC API

	function subscribe(callback)
	{
		subs.push(callback);
	}

	function unsubscribe(callback)
	{
		// copy subs into a new array in case unsubscribe is called within a
		// subscribed callback
		subs = subs.slice();
		var index = subs.indexOf(callback);
		if (index >= 0)
		{
			subs.splice(index, 1);
		}
	}

	return {
		subscribe: subscribe,
		unsubscribe: unsubscribe
	};
}



// INCOMING PORTS


function _Platform_incomingPort(name, converter)
{
	_Platform_checkPortName(name);
	_Platform_effectManagers[name] = {
		f: _Platform_incomingPortMap,
		u: converter,
		a: _Platform_setupIncomingPort
	};
	return _Platform_leaf(name);
}


var _Platform_incomingPortMap = F2(function(tagger, finalTagger)
{
	return function(value)
	{
		return tagger(finalTagger(value));
	};
});


function _Platform_setupIncomingPort(name, sendToApp)
{
	var subs = _List_Nil;
	var converter = _Platform_effectManagers[name].u;

	// CREATE MANAGER

	var init = _Scheduler_succeed(null);

	_Platform_effectManagers[name].b = init;
	_Platform_effectManagers[name].c = F3(function(router, subList, state)
	{
		subs = subList;
		return init;
	});

	// PUBLIC API

	function send(incomingValue)
	{
		var result = A2(_Json_run, converter, _Json_wrap(incomingValue));

		$elm$core$Result$isOk(result) || _Debug_crash(4, name, result.a);

		var value = result.a;
		for (var temp = subs; temp.b; temp = temp.b) // WHILE_CONS
		{
			sendToApp(temp.a(value));
		}
	}

	return { send: send };
}



// EXPORT ELM MODULES
//
// Have DEBUG and PROD versions so that we can (1) give nicer errors in
// debug mode and (2) not pay for the bits needed for that in prod mode.
//


function _Platform_export_UNUSED(exports)
{
	scope['Elm']
		? _Platform_mergeExportsProd(scope['Elm'], exports)
		: scope['Elm'] = exports;
}


function _Platform_mergeExportsProd(obj, exports)
{
	for (var name in exports)
	{
		(name in obj)
			? (name == 'init')
				? _Debug_crash(6)
				: _Platform_mergeExportsProd(obj[name], exports[name])
			: (obj[name] = exports[name]);
	}
}


function _Platform_export(exports)
{
	scope['Elm']
		? _Platform_mergeExportsDebug('Elm', scope['Elm'], exports)
		: scope['Elm'] = exports;
}


function _Platform_mergeExportsDebug(moduleName, obj, exports)
{
	for (var name in exports)
	{
		(name in obj)
			? (name == 'init')
				? _Debug_crash(6, moduleName)
				: _Platform_mergeExportsDebug(moduleName + '.' + name, obj[name], exports[name])
			: (obj[name] = exports[name]);
	}
}




// HELPERS


var _VirtualDom_divertHrefToApp;

var _VirtualDom_doc = typeof document !== 'undefined' ? document : {};


function _VirtualDom_appendChild(parent, child)
{
	parent.appendChild(child);
}

var _VirtualDom_init = F4(function(virtualNode, flagDecoder, debugMetadata, args)
{
	// NOTE: this function needs _Platform_export available to work

	/**_UNUSED/
	var node = args['node'];
	//*/
	/**/
	var node = args && args['node'] ? args['node'] : _Debug_crash(0);
	//*/

	node.parentNode.replaceChild(
		_VirtualDom_render(virtualNode, function() {}),
		node
	);

	return {};
});



// TEXT


function _VirtualDom_text(string)
{
	return {
		$: 0,
		a: string
	};
}



// NODE


var _VirtualDom_nodeNS = F2(function(namespace, tag)
{
	return F2(function(factList, kidList)
	{
		for (var kids = [], descendantsCount = 0; kidList.b; kidList = kidList.b) // WHILE_CONS
		{
			var kid = kidList.a;
			descendantsCount += (kid.b || 0);
			kids.push(kid);
		}
		descendantsCount += kids.length;

		return {
			$: 1,
			c: tag,
			d: _VirtualDom_organizeFacts(factList),
			e: kids,
			f: namespace,
			b: descendantsCount
		};
	});
});


var _VirtualDom_node = _VirtualDom_nodeNS(undefined);



// KEYED NODE


var _VirtualDom_keyedNodeNS = F2(function(namespace, tag)
{
	return F2(function(factList, kidList)
	{
		for (var kids = [], descendantsCount = 0; kidList.b; kidList = kidList.b) // WHILE_CONS
		{
			var kid = kidList.a;
			descendantsCount += (kid.b.b || 0);
			kids.push(kid);
		}
		descendantsCount += kids.length;

		return {
			$: 2,
			c: tag,
			d: _VirtualDom_organizeFacts(factList),
			e: kids,
			f: namespace,
			b: descendantsCount
		};
	});
});


var _VirtualDom_keyedNode = _VirtualDom_keyedNodeNS(undefined);



// CUSTOM


function _VirtualDom_custom(factList, model, render, diff)
{
	return {
		$: 3,
		d: _VirtualDom_organizeFacts(factList),
		g: model,
		h: render,
		i: diff
	};
}



// MAP


var _VirtualDom_map = F2(function(tagger, node)
{
	return {
		$: 4,
		j: tagger,
		k: node,
		b: 1 + (node.b || 0)
	};
});



// LAZY


function _VirtualDom_thunk(refs, thunk)
{
	return {
		$: 5,
		l: refs,
		m: thunk,
		k: undefined
	};
}

var _VirtualDom_lazy = F2(function(func, a)
{
	return _VirtualDom_thunk([func, a], function() {
		return func(a);
	});
});

var _VirtualDom_lazy2 = F3(function(func, a, b)
{
	return _VirtualDom_thunk([func, a, b], function() {
		return A2(func, a, b);
	});
});

var _VirtualDom_lazy3 = F4(function(func, a, b, c)
{
	return _VirtualDom_thunk([func, a, b, c], function() {
		return A3(func, a, b, c);
	});
});

var _VirtualDom_lazy4 = F5(function(func, a, b, c, d)
{
	return _VirtualDom_thunk([func, a, b, c, d], function() {
		return A4(func, a, b, c, d);
	});
});

var _VirtualDom_lazy5 = F6(function(func, a, b, c, d, e)
{
	return _VirtualDom_thunk([func, a, b, c, d, e], function() {
		return A5(func, a, b, c, d, e);
	});
});

var _VirtualDom_lazy6 = F7(function(func, a, b, c, d, e, f)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f], function() {
		return A6(func, a, b, c, d, e, f);
	});
});

var _VirtualDom_lazy7 = F8(function(func, a, b, c, d, e, f, g)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f, g], function() {
		return A7(func, a, b, c, d, e, f, g);
	});
});

var _VirtualDom_lazy8 = F9(function(func, a, b, c, d, e, f, g, h)
{
	return _VirtualDom_thunk([func, a, b, c, d, e, f, g, h], function() {
		return A8(func, a, b, c, d, e, f, g, h);
	});
});



// FACTS


var _VirtualDom_on = F2(function(key, handler)
{
	return {
		$: 'a0',
		n: key,
		o: handler
	};
});
var _VirtualDom_style = F2(function(key, value)
{
	return {
		$: 'a1',
		n: key,
		o: value
	};
});
var _VirtualDom_property = F2(function(key, value)
{
	return {
		$: 'a2',
		n: key,
		o: value
	};
});
var _VirtualDom_attribute = F2(function(key, value)
{
	return {
		$: 'a3',
		n: key,
		o: value
	};
});
var _VirtualDom_attributeNS = F3(function(namespace, key, value)
{
	return {
		$: 'a4',
		n: key,
		o: { f: namespace, o: value }
	};
});



// XSS ATTACK VECTOR CHECKS
//
// For some reason, tabs can appear in href protocols and it still works.
// So '\tjava\tSCRIPT:alert("!!!")' and 'javascript:alert("!!!")' are the same
// in practice. That is why _VirtualDom_RE_js and _VirtualDom_RE_js_html look
// so freaky.
//
// Pulling the regular expressions out to the top level gives a slight speed
// boost in small benchmarks (4-10%) but hoisting values to reduce allocation
// can be unpredictable in large programs where JIT may have a harder time with
// functions are not fully self-contained. The benefit is more that the js and
// js_html ones are so weird that I prefer to see them near each other.


var _VirtualDom_RE_script = /^script$/i;
var _VirtualDom_RE_on_formAction = /^(on|formAction$)/i;
var _VirtualDom_RE_js = /^\s*j\s*a\s*v\s*a\s*s\s*c\s*r\s*i\s*p\s*t\s*:/i;
var _VirtualDom_RE_js_html = /^\s*(j\s*a\s*v\s*a\s*s\s*c\s*r\s*i\s*p\s*t\s*:|d\s*a\s*t\s*a\s*:\s*t\s*e\s*x\s*t\s*\/\s*h\s*t\s*m\s*l\s*(,|;))/i;


function _VirtualDom_noScript(tag)
{
	return _VirtualDom_RE_script.test(tag) ? 'p' : tag;
}

function _VirtualDom_noOnOrFormAction(key)
{
	return _VirtualDom_RE_on_formAction.test(key) ? 'data-' + key : key;
}

function _VirtualDom_noInnerHtmlOrFormAction(key)
{
	return key == 'innerHTML' || key == 'outerHTML' || key == 'formAction' ? 'data-' + key : key;
}

function _VirtualDom_noJavaScriptUri(value)
{
	return _VirtualDom_RE_js.test(value)
		? /**_UNUSED/''//*//**/'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'//*/
		: value;
}

function _VirtualDom_noJavaScriptOrHtmlUri(value)
{
	return _VirtualDom_RE_js_html.test(value)
		? /**_UNUSED/''//*//**/'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'//*/
		: value;
}

function _VirtualDom_noJavaScriptOrHtmlJson(value)
{
	return (
		(typeof _Json_unwrap(value) === 'string' && _VirtualDom_RE_js_html.test(_Json_unwrap(value)))
		||
		(Array.isArray(_Json_unwrap(value)) && _VirtualDom_RE_js_html.test(String(_Json_unwrap(value))))
	)
		? _Json_wrap(
			/**_UNUSED/''//*//**/'javascript:alert("This is an XSS vector. Please use ports or web components instead.")'//*/
		) : value;
}



// MAP FACTS


var _VirtualDom_mapAttribute = F2(function(func, attr)
{
	return (attr.$ === 'a0')
		? A2(_VirtualDom_on, attr.n, _VirtualDom_mapHandler(func, attr.o))
		: attr;
});

function _VirtualDom_mapHandler(func, handler)
{
	var tag = $elm$virtual_dom$VirtualDom$toHandlerInt(handler);

	// 0 = Normal
	// 1 = MayStopPropagation
	// 2 = MayPreventDefault
	// 3 = Custom

	return {
		$: handler.$,
		a:
			!tag
				? A2($elm$json$Json$Decode$map, func, handler.a)
				:
			A3($elm$json$Json$Decode$map2,
				tag < 3
					? _VirtualDom_mapEventTuple
					: _VirtualDom_mapEventRecord,
				$elm$json$Json$Decode$succeed(func),
				handler.a
			)
	};
}

var _VirtualDom_mapEventTuple = F2(function(func, tuple)
{
	return _Utils_Tuple2(func(tuple.a), tuple.b);
});

var _VirtualDom_mapEventRecord = F2(function(func, record)
{
	return {
		message: func(record.message),
		stopPropagation: record.stopPropagation,
		preventDefault: record.preventDefault
	}
});



// ORGANIZE FACTS


function _VirtualDom_organizeFacts(factList)
{
	for (var facts = {}; factList.b; factList = factList.b) // WHILE_CONS
	{
		var entry = factList.a;

		var tag = entry.$;
		var key = entry.n;
		var value = entry.o;

		if (tag === 'a2')
		{
			(key === 'className')
				? _VirtualDom_addClass(facts, key, _Json_unwrap(value))
				: facts[key] = _Json_unwrap(value);

			continue;
		}

		var subFacts = facts[tag] || (facts[tag] = {});
		(tag === 'a3' && key === 'class')
			? _VirtualDom_addClass(subFacts, key, value)
			: subFacts[key] = value;
	}

	return facts;
}

function _VirtualDom_addClass(object, key, newClass)
{
	var classes = object[key];
	object[key] = classes ? classes + ' ' + newClass : newClass;
}



// RENDER


function _VirtualDom_render(vNode, eventNode)
{
	var tag = vNode.$;

	if (tag === 5)
	{
		return _VirtualDom_render(vNode.k || (vNode.k = vNode.m()), eventNode);
	}

	if (tag === 0)
	{
		return _VirtualDom_doc.createTextNode(vNode.a);
	}

	if (tag === 4)
	{
		var subNode = vNode.k;
		var tagger = vNode.j;

		while (subNode.$ === 4)
		{
			typeof tagger !== 'object'
				? tagger = [tagger, subNode.j]
				: tagger.push(subNode.j);

			subNode = subNode.k;
		}

		var subEventRoot = { j: tagger, p: eventNode };
		var domNode = _VirtualDom_render(subNode, subEventRoot);
		domNode.elm_event_node_ref = subEventRoot;
		return domNode;
	}

	if (tag === 3)
	{
		var domNode = vNode.h(vNode.g);
		_VirtualDom_applyFacts(domNode, eventNode, vNode.d);
		return domNode;
	}

	// at this point `tag` must be 1 or 2

	var domNode = vNode.f
		? _VirtualDom_doc.createElementNS(vNode.f, vNode.c)
		: _VirtualDom_doc.createElement(vNode.c);

	if (_VirtualDom_divertHrefToApp && vNode.c == 'a')
	{
		domNode.addEventListener('click', _VirtualDom_divertHrefToApp(domNode));
	}

	_VirtualDom_applyFacts(domNode, eventNode, vNode.d);

	for (var kids = vNode.e, i = 0; i < kids.length; i++)
	{
		_VirtualDom_appendChild(domNode, _VirtualDom_render(tag === 1 ? kids[i] : kids[i].b, eventNode));
	}

	return domNode;
}



// APPLY FACTS


function _VirtualDom_applyFacts(domNode, eventNode, facts)
{
	for (var key in facts)
	{
		var value = facts[key];

		key === 'a1'
			? _VirtualDom_applyStyles(domNode, value)
			:
		key === 'a0'
			? _VirtualDom_applyEvents(domNode, eventNode, value)
			:
		key === 'a3'
			? _VirtualDom_applyAttrs(domNode, value)
			:
		key === 'a4'
			? _VirtualDom_applyAttrsNS(domNode, value)
			:
		((key !== 'value' && key !== 'checked') || domNode[key] !== value) && (domNode[key] = value);
	}
}



// APPLY STYLES


function _VirtualDom_applyStyles(domNode, styles)
{
	var domNodeStyle = domNode.style;

	for (var key in styles)
	{
		domNodeStyle[key] = styles[key];
	}
}



// APPLY ATTRS


function _VirtualDom_applyAttrs(domNode, attrs)
{
	for (var key in attrs)
	{
		var value = attrs[key];
		typeof value !== 'undefined'
			? domNode.setAttribute(key, value)
			: domNode.removeAttribute(key);
	}
}



// APPLY NAMESPACED ATTRS


function _VirtualDom_applyAttrsNS(domNode, nsAttrs)
{
	for (var key in nsAttrs)
	{
		var pair = nsAttrs[key];
		var namespace = pair.f;
		var value = pair.o;

		typeof value !== 'undefined'
			? domNode.setAttributeNS(namespace, key, value)
			: domNode.removeAttributeNS(namespace, key);
	}
}



// APPLY EVENTS


function _VirtualDom_applyEvents(domNode, eventNode, events)
{
	var allCallbacks = domNode.elmFs || (domNode.elmFs = {});

	for (var key in events)
	{
		var newHandler = events[key];
		var oldCallback = allCallbacks[key];

		if (!newHandler)
		{
			domNode.removeEventListener(key, oldCallback);
			allCallbacks[key] = undefined;
			continue;
		}

		if (oldCallback)
		{
			var oldHandler = oldCallback.q;
			if (oldHandler.$ === newHandler.$)
			{
				oldCallback.q = newHandler;
				continue;
			}
			domNode.removeEventListener(key, oldCallback);
		}

		oldCallback = _VirtualDom_makeCallback(eventNode, newHandler);
		domNode.addEventListener(key, oldCallback,
			_VirtualDom_passiveSupported
			&& { passive: $elm$virtual_dom$VirtualDom$toHandlerInt(newHandler) < 2 }
		);
		allCallbacks[key] = oldCallback;
	}
}



// PASSIVE EVENTS


var _VirtualDom_passiveSupported;

try
{
	window.addEventListener('t', null, Object.defineProperty({}, 'passive', {
		get: function() { _VirtualDom_passiveSupported = true; }
	}));
}
catch(e) {}



// EVENT HANDLERS


function _VirtualDom_makeCallback(eventNode, initialHandler)
{
	function callback(event)
	{
		var handler = callback.q;
		var result = _Json_runHelp(handler.a, event);

		if (!$elm$core$Result$isOk(result))
		{
			return;
		}

		var tag = $elm$virtual_dom$VirtualDom$toHandlerInt(handler);

		// 0 = Normal
		// 1 = MayStopPropagation
		// 2 = MayPreventDefault
		// 3 = Custom

		var value = result.a;
		var message = !tag ? value : tag < 3 ? value.a : value.message;
		var stopPropagation = tag == 1 ? value.b : tag == 3 && value.stopPropagation;
		var currentEventNode = (
			stopPropagation && event.stopPropagation(),
			(tag == 2 ? value.b : tag == 3 && value.preventDefault) && event.preventDefault(),
			eventNode
		);
		var tagger;
		var i;
		while (tagger = currentEventNode.j)
		{
			if (typeof tagger == 'function')
			{
				message = tagger(message);
			}
			else
			{
				for (var i = tagger.length; i--; )
				{
					message = tagger[i](message);
				}
			}
			currentEventNode = currentEventNode.p;
		}
		currentEventNode(message, stopPropagation); // stopPropagation implies isSync
	}

	callback.q = initialHandler;

	return callback;
}

function _VirtualDom_equalEvents(x, y)
{
	return x.$ == y.$ && _Json_equality(x.a, y.a);
}



// DIFF


// TODO: Should we do patches like in iOS?
//
// type Patch
//   = At Int Patch
//   | Batch (List Patch)
//   | Change ...
//
// How could it not be better?
//
function _VirtualDom_diff(x, y)
{
	var patches = [];
	_VirtualDom_diffHelp(x, y, patches, 0);
	return patches;
}


function _VirtualDom_pushPatch(patches, type, index, data)
{
	var patch = {
		$: type,
		r: index,
		s: data,
		t: undefined,
		u: undefined
	};
	patches.push(patch);
	return patch;
}


function _VirtualDom_diffHelp(x, y, patches, index)
{
	if (x === y)
	{
		return;
	}

	var xType = x.$;
	var yType = y.$;

	// Bail if you run into different types of nodes. Implies that the
	// structure has changed significantly and it's not worth a diff.
	if (xType !== yType)
	{
		if (xType === 1 && yType === 2)
		{
			y = _VirtualDom_dekey(y);
			yType = 1;
		}
		else
		{
			_VirtualDom_pushPatch(patches, 0, index, y);
			return;
		}
	}

	// Now we know that both nodes are the same $.
	switch (yType)
	{
		case 5:
			var xRefs = x.l;
			var yRefs = y.l;
			var i = xRefs.length;
			var same = i === yRefs.length;
			while (same && i--)
			{
				same = xRefs[i] === yRefs[i];
			}
			if (same)
			{
				y.k = x.k;
				return;
			}
			y.k = y.m();
			var subPatches = [];
			_VirtualDom_diffHelp(x.k, y.k, subPatches, 0);
			subPatches.length > 0 && _VirtualDom_pushPatch(patches, 1, index, subPatches);
			return;

		case 4:
			// gather nested taggers
			var xTaggers = x.j;
			var yTaggers = y.j;
			var nesting = false;

			var xSubNode = x.k;
			while (xSubNode.$ === 4)
			{
				nesting = true;

				typeof xTaggers !== 'object'
					? xTaggers = [xTaggers, xSubNode.j]
					: xTaggers.push(xSubNode.j);

				xSubNode = xSubNode.k;
			}

			var ySubNode = y.k;
			while (ySubNode.$ === 4)
			{
				nesting = true;

				typeof yTaggers !== 'object'
					? yTaggers = [yTaggers, ySubNode.j]
					: yTaggers.push(ySubNode.j);

				ySubNode = ySubNode.k;
			}

			// Just bail if different numbers of taggers. This implies the
			// structure of the virtual DOM has changed.
			if (nesting && xTaggers.length !== yTaggers.length)
			{
				_VirtualDom_pushPatch(patches, 0, index, y);
				return;
			}

			// check if taggers are "the same"
			if (nesting ? !_VirtualDom_pairwiseRefEqual(xTaggers, yTaggers) : xTaggers !== yTaggers)
			{
				_VirtualDom_pushPatch(patches, 2, index, yTaggers);
			}

			// diff everything below the taggers
			_VirtualDom_diffHelp(xSubNode, ySubNode, patches, index + 1);
			return;

		case 0:
			if (x.a !== y.a)
			{
				_VirtualDom_pushPatch(patches, 3, index, y.a);
			}
			return;

		case 1:
			_VirtualDom_diffNodes(x, y, patches, index, _VirtualDom_diffKids);
			return;

		case 2:
			_VirtualDom_diffNodes(x, y, patches, index, _VirtualDom_diffKeyedKids);
			return;

		case 3:
			if (x.h !== y.h)
			{
				_VirtualDom_pushPatch(patches, 0, index, y);
				return;
			}

			var factsDiff = _VirtualDom_diffFacts(x.d, y.d);
			factsDiff && _VirtualDom_pushPatch(patches, 4, index, factsDiff);

			var patch = y.i(x.g, y.g);
			patch && _VirtualDom_pushPatch(patches, 5, index, patch);

			return;
	}
}

// assumes the incoming arrays are the same length
function _VirtualDom_pairwiseRefEqual(as, bs)
{
	for (var i = 0; i < as.length; i++)
	{
		if (as[i] !== bs[i])
		{
			return false;
		}
	}

	return true;
}

function _VirtualDom_diffNodes(x, y, patches, index, diffKids)
{
	// Bail if obvious indicators have changed. Implies more serious
	// structural changes such that it's not worth it to diff.
	if (x.c !== y.c || x.f !== y.f)
	{
		_VirtualDom_pushPatch(patches, 0, index, y);
		return;
	}

	var factsDiff = _VirtualDom_diffFacts(x.d, y.d);
	factsDiff && _VirtualDom_pushPatch(patches, 4, index, factsDiff);

	diffKids(x, y, patches, index);
}



// DIFF FACTS


// TODO Instead of creating a new diff object, it's possible to just test if
// there *is* a diff. During the actual patch, do the diff again and make the
// modifications directly. This way, there's no new allocations. Worth it?
function _VirtualDom_diffFacts(x, y, category)
{
	var diff;

	// look for changes and removals
	for (var xKey in x)
	{
		if (xKey === 'a1' || xKey === 'a0' || xKey === 'a3' || xKey === 'a4')
		{
			var subDiff = _VirtualDom_diffFacts(x[xKey], y[xKey] || {}, xKey);
			if (subDiff)
			{
				diff = diff || {};
				diff[xKey] = subDiff;
			}
			continue;
		}

		// remove if not in the new facts
		if (!(xKey in y))
		{
			diff = diff || {};
			diff[xKey] =
				!category
					? (typeof x[xKey] === 'string' ? '' : null)
					:
				(category === 'a1')
					? ''
					:
				(category === 'a0' || category === 'a3')
					? undefined
					:
				{ f: x[xKey].f, o: undefined };

			continue;
		}

		var xValue = x[xKey];
		var yValue = y[xKey];

		// reference equal, so don't worry about it
		if (xValue === yValue && xKey !== 'value' && xKey !== 'checked'
			|| category === 'a0' && _VirtualDom_equalEvents(xValue, yValue))
		{
			continue;
		}

		diff = diff || {};
		diff[xKey] = yValue;
	}

	// add new stuff
	for (var yKey in y)
	{
		if (!(yKey in x))
		{
			diff = diff || {};
			diff[yKey] = y[yKey];
		}
	}

	return diff;
}



// DIFF KIDS


function _VirtualDom_diffKids(xParent, yParent, patches, index)
{
	var xKids = xParent.e;
	var yKids = yParent.e;

	var xLen = xKids.length;
	var yLen = yKids.length;

	// FIGURE OUT IF THERE ARE INSERTS OR REMOVALS

	if (xLen > yLen)
	{
		_VirtualDom_pushPatch(patches, 6, index, {
			v: yLen,
			i: xLen - yLen
		});
	}
	else if (xLen < yLen)
	{
		_VirtualDom_pushPatch(patches, 7, index, {
			v: xLen,
			e: yKids
		});
	}

	// PAIRWISE DIFF EVERYTHING ELSE

	for (var minLen = xLen < yLen ? xLen : yLen, i = 0; i < minLen; i++)
	{
		var xKid = xKids[i];
		_VirtualDom_diffHelp(xKid, yKids[i], patches, ++index);
		index += xKid.b || 0;
	}
}



// KEYED DIFF


function _VirtualDom_diffKeyedKids(xParent, yParent, patches, rootIndex)
{
	var localPatches = [];

	var changes = {}; // Dict String Entry
	var inserts = []; // Array { index : Int, entry : Entry }
	// type Entry = { tag : String, vnode : VNode, index : Int, data : _ }

	var xKids = xParent.e;
	var yKids = yParent.e;
	var xLen = xKids.length;
	var yLen = yKids.length;
	var xIndex = 0;
	var yIndex = 0;

	var index = rootIndex;

	while (xIndex < xLen && yIndex < yLen)
	{
		var x = xKids[xIndex];
		var y = yKids[yIndex];

		var xKey = x.a;
		var yKey = y.a;
		var xNode = x.b;
		var yNode = y.b;

		var newMatch = undefined;
		var oldMatch = undefined;

		// check if keys match

		if (xKey === yKey)
		{
			index++;
			_VirtualDom_diffHelp(xNode, yNode, localPatches, index);
			index += xNode.b || 0;

			xIndex++;
			yIndex++;
			continue;
		}

		// look ahead 1 to detect insertions and removals.

		var xNext = xKids[xIndex + 1];
		var yNext = yKids[yIndex + 1];

		if (xNext)
		{
			var xNextKey = xNext.a;
			var xNextNode = xNext.b;
			oldMatch = yKey === xNextKey;
		}

		if (yNext)
		{
			var yNextKey = yNext.a;
			var yNextNode = yNext.b;
			newMatch = xKey === yNextKey;
		}


		// swap x and y
		if (newMatch && oldMatch)
		{
			index++;
			_VirtualDom_diffHelp(xNode, yNextNode, localPatches, index);
			_VirtualDom_insertNode(changes, localPatches, xKey, yNode, yIndex, inserts);
			index += xNode.b || 0;

			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNextNode, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 2;
			continue;
		}

		// insert y
		if (newMatch)
		{
			index++;
			_VirtualDom_insertNode(changes, localPatches, yKey, yNode, yIndex, inserts);
			_VirtualDom_diffHelp(xNode, yNextNode, localPatches, index);
			index += xNode.b || 0;

			xIndex += 1;
			yIndex += 2;
			continue;
		}

		// remove x
		if (oldMatch)
		{
			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNode, index);
			index += xNode.b || 0;

			index++;
			_VirtualDom_diffHelp(xNextNode, yNode, localPatches, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 1;
			continue;
		}

		// remove x, insert y
		if (xNext && xNextKey === yNextKey)
		{
			index++;
			_VirtualDom_removeNode(changes, localPatches, xKey, xNode, index);
			_VirtualDom_insertNode(changes, localPatches, yKey, yNode, yIndex, inserts);
			index += xNode.b || 0;

			index++;
			_VirtualDom_diffHelp(xNextNode, yNextNode, localPatches, index);
			index += xNextNode.b || 0;

			xIndex += 2;
			yIndex += 2;
			continue;
		}

		break;
	}

	// eat up any remaining nodes with removeNode and insertNode

	while (xIndex < xLen)
	{
		index++;
		var x = xKids[xIndex];
		var xNode = x.b;
		_VirtualDom_removeNode(changes, localPatches, x.a, xNode, index);
		index += xNode.b || 0;
		xIndex++;
	}

	while (yIndex < yLen)
	{
		var endInserts = endInserts || [];
		var y = yKids[yIndex];
		_VirtualDom_insertNode(changes, localPatches, y.a, y.b, undefined, endInserts);
		yIndex++;
	}

	if (localPatches.length > 0 || inserts.length > 0 || endInserts)
	{
		_VirtualDom_pushPatch(patches, 8, rootIndex, {
			w: localPatches,
			x: inserts,
			y: endInserts
		});
	}
}



// CHANGES FROM KEYED DIFF


var _VirtualDom_POSTFIX = '_elmW6BL';


function _VirtualDom_insertNode(changes, localPatches, key, vnode, yIndex, inserts)
{
	var entry = changes[key];

	// never seen this key before
	if (!entry)
	{
		entry = {
			c: 0,
			z: vnode,
			r: yIndex,
			s: undefined
		};

		inserts.push({ r: yIndex, A: entry });
		changes[key] = entry;

		return;
	}

	// this key was removed earlier, a match!
	if (entry.c === 1)
	{
		inserts.push({ r: yIndex, A: entry });

		entry.c = 2;
		var subPatches = [];
		_VirtualDom_diffHelp(entry.z, vnode, subPatches, entry.r);
		entry.r = yIndex;
		entry.s.s = {
			w: subPatches,
			A: entry
		};

		return;
	}

	// this key has already been inserted or moved, a duplicate!
	_VirtualDom_insertNode(changes, localPatches, key + _VirtualDom_POSTFIX, vnode, yIndex, inserts);
}


function _VirtualDom_removeNode(changes, localPatches, key, vnode, index)
{
	var entry = changes[key];

	// never seen this key before
	if (!entry)
	{
		var patch = _VirtualDom_pushPatch(localPatches, 9, index, undefined);

		changes[key] = {
			c: 1,
			z: vnode,
			r: index,
			s: patch
		};

		return;
	}

	// this key was inserted earlier, a match!
	if (entry.c === 0)
	{
		entry.c = 2;
		var subPatches = [];
		_VirtualDom_diffHelp(vnode, entry.z, subPatches, index);

		_VirtualDom_pushPatch(localPatches, 9, index, {
			w: subPatches,
			A: entry
		});

		return;
	}

	// this key has already been removed or moved, a duplicate!
	_VirtualDom_removeNode(changes, localPatches, key + _VirtualDom_POSTFIX, vnode, index);
}



// ADD DOM NODES
//
// Each DOM node has an "index" assigned in order of traversal. It is important
// to minimize our crawl over the actual DOM, so these indexes (along with the
// descendantsCount of virtual nodes) let us skip touching entire subtrees of
// the DOM if we know there are no patches there.


function _VirtualDom_addDomNodes(domNode, vNode, patches, eventNode)
{
	_VirtualDom_addDomNodesHelp(domNode, vNode, patches, 0, 0, vNode.b, eventNode);
}


// assumes `patches` is non-empty and indexes increase monotonically.
function _VirtualDom_addDomNodesHelp(domNode, vNode, patches, i, low, high, eventNode)
{
	var patch = patches[i];
	var index = patch.r;

	while (index === low)
	{
		var patchType = patch.$;

		if (patchType === 1)
		{
			_VirtualDom_addDomNodes(domNode, vNode.k, patch.s, eventNode);
		}
		else if (patchType === 8)
		{
			patch.t = domNode;
			patch.u = eventNode;

			var subPatches = patch.s.w;
			if (subPatches.length > 0)
			{
				_VirtualDom_addDomNodesHelp(domNode, vNode, subPatches, 0, low, high, eventNode);
			}
		}
		else if (patchType === 9)
		{
			patch.t = domNode;
			patch.u = eventNode;

			var data = patch.s;
			if (data)
			{
				data.A.s = domNode;
				var subPatches = data.w;
				if (subPatches.length > 0)
				{
					_VirtualDom_addDomNodesHelp(domNode, vNode, subPatches, 0, low, high, eventNode);
				}
			}
		}
		else
		{
			patch.t = domNode;
			patch.u = eventNode;
		}

		i++;

		if (!(patch = patches[i]) || (index = patch.r) > high)
		{
			return i;
		}
	}

	var tag = vNode.$;

	if (tag === 4)
	{
		var subNode = vNode.k;

		while (subNode.$ === 4)
		{
			subNode = subNode.k;
		}

		return _VirtualDom_addDomNodesHelp(domNode, subNode, patches, i, low + 1, high, domNode.elm_event_node_ref);
	}

	// tag must be 1 or 2 at this point

	var vKids = vNode.e;
	var childNodes = domNode.childNodes;
	for (var j = 0; j < vKids.length; j++)
	{
		low++;
		var vKid = tag === 1 ? vKids[j] : vKids[j].b;
		var nextLow = low + (vKid.b || 0);
		if (low <= index && index <= nextLow)
		{
			i = _VirtualDom_addDomNodesHelp(childNodes[j], vKid, patches, i, low, nextLow, eventNode);
			if (!(patch = patches[i]) || (index = patch.r) > high)
			{
				return i;
			}
		}
		low = nextLow;
	}
	return i;
}



// APPLY PATCHES


function _VirtualDom_applyPatches(rootDomNode, oldVirtualNode, patches, eventNode)
{
	if (patches.length === 0)
	{
		return rootDomNode;
	}

	_VirtualDom_addDomNodes(rootDomNode, oldVirtualNode, patches, eventNode);
	return _VirtualDom_applyPatchesHelp(rootDomNode, patches);
}

function _VirtualDom_applyPatchesHelp(rootDomNode, patches)
{
	for (var i = 0; i < patches.length; i++)
	{
		var patch = patches[i];
		var localDomNode = patch.t
		var newNode = _VirtualDom_applyPatch(localDomNode, patch);
		if (localDomNode === rootDomNode)
		{
			rootDomNode = newNode;
		}
	}
	return rootDomNode;
}

function _VirtualDom_applyPatch(domNode, patch)
{
	switch (patch.$)
	{
		case 0:
			return _VirtualDom_applyPatchRedraw(domNode, patch.s, patch.u);

		case 4:
			_VirtualDom_applyFacts(domNode, patch.u, patch.s);
			return domNode;

		case 3:
			domNode.replaceData(0, domNode.length, patch.s);
			return domNode;

		case 1:
			return _VirtualDom_applyPatchesHelp(domNode, patch.s);

		case 2:
			if (domNode.elm_event_node_ref)
			{
				domNode.elm_event_node_ref.j = patch.s;
			}
			else
			{
				domNode.elm_event_node_ref = { j: patch.s, p: patch.u };
			}
			return domNode;

		case 6:
			var data = patch.s;
			for (var i = 0; i < data.i; i++)
			{
				domNode.removeChild(domNode.childNodes[data.v]);
			}
			return domNode;

		case 7:
			var data = patch.s;
			var kids = data.e;
			var i = data.v;
			var theEnd = domNode.childNodes[i];
			for (; i < kids.length; i++)
			{
				domNode.insertBefore(_VirtualDom_render(kids[i], patch.u), theEnd);
			}
			return domNode;

		case 9:
			var data = patch.s;
			if (!data)
			{
				domNode.parentNode.removeChild(domNode);
				return domNode;
			}
			var entry = data.A;
			if (typeof entry.r !== 'undefined')
			{
				domNode.parentNode.removeChild(domNode);
			}
			entry.s = _VirtualDom_applyPatchesHelp(domNode, data.w);
			return domNode;

		case 8:
			return _VirtualDom_applyPatchReorder(domNode, patch);

		case 5:
			return patch.s(domNode);

		default:
			_Debug_crash(10); // 'Ran into an unknown patch!'
	}
}


function _VirtualDom_applyPatchRedraw(domNode, vNode, eventNode)
{
	var parentNode = domNode.parentNode;
	var newNode = _VirtualDom_render(vNode, eventNode);

	if (!newNode.elm_event_node_ref)
	{
		newNode.elm_event_node_ref = domNode.elm_event_node_ref;
	}

	if (parentNode && newNode !== domNode)
	{
		parentNode.replaceChild(newNode, domNode);
	}
	return newNode;
}


function _VirtualDom_applyPatchReorder(domNode, patch)
{
	var data = patch.s;

	// remove end inserts
	var frag = _VirtualDom_applyPatchReorderEndInsertsHelp(data.y, patch);

	// removals
	domNode = _VirtualDom_applyPatchesHelp(domNode, data.w);

	// inserts
	var inserts = data.x;
	for (var i = 0; i < inserts.length; i++)
	{
		var insert = inserts[i];
		var entry = insert.A;
		var node = entry.c === 2
			? entry.s
			: _VirtualDom_render(entry.z, patch.u);
		domNode.insertBefore(node, domNode.childNodes[insert.r]);
	}

	// add end inserts
	if (frag)
	{
		_VirtualDom_appendChild(domNode, frag);
	}

	return domNode;
}


function _VirtualDom_applyPatchReorderEndInsertsHelp(endInserts, patch)
{
	if (!endInserts)
	{
		return;
	}

	var frag = _VirtualDom_doc.createDocumentFragment();
	for (var i = 0; i < endInserts.length; i++)
	{
		var insert = endInserts[i];
		var entry = insert.A;
		_VirtualDom_appendChild(frag, entry.c === 2
			? entry.s
			: _VirtualDom_render(entry.z, patch.u)
		);
	}
	return frag;
}


function _VirtualDom_virtualize(node)
{
	// TEXT NODES

	if (node.nodeType === 3)
	{
		return _VirtualDom_text(node.textContent);
	}


	// WEIRD NODES

	if (node.nodeType !== 1)
	{
		return _VirtualDom_text('');
	}


	// ELEMENT NODES

	var attrList = _List_Nil;
	var attrs = node.attributes;
	for (var i = attrs.length; i--; )
	{
		var attr = attrs[i];
		var name = attr.name;
		var value = attr.value;
		attrList = _List_Cons( A2(_VirtualDom_attribute, name, value), attrList );
	}

	var tag = node.tagName.toLowerCase();
	var kidList = _List_Nil;
	var kids = node.childNodes;

	for (var i = kids.length; i--; )
	{
		kidList = _List_Cons(_VirtualDom_virtualize(kids[i]), kidList);
	}
	return A3(_VirtualDom_node, tag, attrList, kidList);
}

function _VirtualDom_dekey(keyedNode)
{
	var keyedKids = keyedNode.e;
	var len = keyedKids.length;
	var kids = new Array(len);
	for (var i = 0; i < len; i++)
	{
		kids[i] = keyedKids[i].b;
	}

	return {
		$: 1,
		c: keyedNode.c,
		d: keyedNode.d,
		e: kids,
		f: keyedNode.f,
		b: keyedNode.b
	};
}




// ELEMENT


var _Debugger_element;

var _Browser_element = _Debugger_element || F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.init,
		impl.update,
		impl.subscriptions,
		function(sendToApp, initialModel) {
			var view = impl.view;
			/**_UNUSED/
			var domNode = args['node'];
			//*/
			/**/
			var domNode = args && args['node'] ? args['node'] : _Debug_crash(0);
			//*/
			var currNode = _VirtualDom_virtualize(domNode);

			return _Browser_makeAnimator(initialModel, function(model)
			{
				var nextNode = view(model);
				var patches = _VirtualDom_diff(currNode, nextNode);
				domNode = _VirtualDom_applyPatches(domNode, currNode, patches, sendToApp);
				currNode = nextNode;
			});
		}
	);
});



// DOCUMENT


var _Debugger_document;

var _Browser_document = _Debugger_document || F4(function(impl, flagDecoder, debugMetadata, args)
{
	return _Platform_initialize(
		flagDecoder,
		args,
		impl.init,
		impl.update,
		impl.subscriptions,
		function(sendToApp, initialModel) {
			var divertHrefToApp = impl.setup && impl.setup(sendToApp)
			var view = impl.view;
			var title = _VirtualDom_doc.title;
			var bodyNode = _VirtualDom_doc.body;
			var currNode = _VirtualDom_virtualize(bodyNode);
			return _Browser_makeAnimator(initialModel, function(model)
			{
				_VirtualDom_divertHrefToApp = divertHrefToApp;
				var doc = view(model);
				var nextNode = _VirtualDom_node('body')(_List_Nil)(doc.body);
				var patches = _VirtualDom_diff(currNode, nextNode);
				bodyNode = _VirtualDom_applyPatches(bodyNode, currNode, patches, sendToApp);
				currNode = nextNode;
				_VirtualDom_divertHrefToApp = 0;
				(title !== doc.title) && (_VirtualDom_doc.title = title = doc.title);
			});
		}
	);
});



// ANIMATION


var _Browser_cancelAnimationFrame =
	typeof cancelAnimationFrame !== 'undefined'
		? cancelAnimationFrame
		: function(id) { clearTimeout(id); };

var _Browser_requestAnimationFrame =
	typeof requestAnimationFrame !== 'undefined'
		? requestAnimationFrame
		: function(callback) { return setTimeout(callback, 1000 / 60); };


function _Browser_makeAnimator(model, draw)
{
	draw(model);

	var state = 0;

	function updateIfNeeded()
	{
		state = state === 1
			? 0
			: ( _Browser_requestAnimationFrame(updateIfNeeded), draw(model), 1 );
	}

	return function(nextModel, isSync)
	{
		model = nextModel;

		isSync
			? ( draw(model),
				state === 2 && (state = 1)
				)
			: ( state === 0 && _Browser_requestAnimationFrame(updateIfNeeded),
				state = 2
				);
	};
}



// APPLICATION


function _Browser_application(impl)
{
	var onUrlChange = impl.onUrlChange;
	var onUrlRequest = impl.onUrlRequest;
	var key = function() { key.a(onUrlChange(_Browser_getUrl())); };

	return _Browser_document({
		setup: function(sendToApp)
		{
			key.a = sendToApp;
			_Browser_window.addEventListener('popstate', key);
			_Browser_window.navigator.userAgent.indexOf('Trident') < 0 || _Browser_window.addEventListener('hashchange', key);

			return F2(function(domNode, event)
			{
				if (!event.ctrlKey && !event.metaKey && !event.shiftKey && event.button < 1 && !domNode.target && !domNode.hasAttribute('download'))
				{
					event.preventDefault();
					var href = domNode.href;
					var curr = _Browser_getUrl();
					var next = $elm$url$Url$fromString(href).a;
					sendToApp(onUrlRequest(
						(next
							&& curr.protocol === next.protocol
							&& curr.host === next.host
							&& curr.port_.a === next.port_.a
						)
							? $elm$browser$Browser$Internal(next)
							: $elm$browser$Browser$External(href)
					));
				}
			});
		},
		init: function(flags)
		{
			return A3(impl.init, flags, _Browser_getUrl(), key);
		},
		view: impl.view,
		update: impl.update,
		subscriptions: impl.subscriptions
	});
}

function _Browser_getUrl()
{
	return $elm$url$Url$fromString(_VirtualDom_doc.location.href).a || _Debug_crash(1);
}

var _Browser_go = F2(function(key, n)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		n && history.go(n);
		key();
	}));
});

var _Browser_pushUrl = F2(function(key, url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		history.pushState({}, '', url);
		key();
	}));
});

var _Browser_replaceUrl = F2(function(key, url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function() {
		history.replaceState({}, '', url);
		key();
	}));
});



// GLOBAL EVENTS


var _Browser_fakeNode = { addEventListener: function() {}, removeEventListener: function() {} };
var _Browser_doc = typeof document !== 'undefined' ? document : _Browser_fakeNode;
var _Browser_window = typeof window !== 'undefined' ? window : _Browser_fakeNode;

var _Browser_on = F3(function(node, eventName, sendToSelf)
{
	return _Scheduler_spawn(_Scheduler_binding(function(callback)
	{
		function handler(event)	{ _Scheduler_rawSpawn(sendToSelf(event)); }
		node.addEventListener(eventName, handler, _VirtualDom_passiveSupported && { passive: true });
		return function() { node.removeEventListener(eventName, handler); };
	}));
});

var _Browser_decodeEvent = F2(function(decoder, event)
{
	var result = _Json_runHelp(decoder, event);
	return $elm$core$Result$isOk(result) ? $elm$core$Maybe$Just(result.a) : $elm$core$Maybe$Nothing;
});



// PAGE VISIBILITY


function _Browser_visibilityInfo()
{
	return (typeof _VirtualDom_doc.hidden !== 'undefined')
		? { hidden: 'hidden', change: 'visibilitychange' }
		:
	(typeof _VirtualDom_doc.mozHidden !== 'undefined')
		? { hidden: 'mozHidden', change: 'mozvisibilitychange' }
		:
	(typeof _VirtualDom_doc.msHidden !== 'undefined')
		? { hidden: 'msHidden', change: 'msvisibilitychange' }
		:
	(typeof _VirtualDom_doc.webkitHidden !== 'undefined')
		? { hidden: 'webkitHidden', change: 'webkitvisibilitychange' }
		: { hidden: 'hidden', change: 'visibilitychange' };
}



// ANIMATION FRAMES


function _Browser_rAF()
{
	return _Scheduler_binding(function(callback)
	{
		var id = _Browser_requestAnimationFrame(function() {
			callback(_Scheduler_succeed(Date.now()));
		});

		return function() {
			_Browser_cancelAnimationFrame(id);
		};
	});
}


function _Browser_now()
{
	return _Scheduler_binding(function(callback)
	{
		callback(_Scheduler_succeed(Date.now()));
	});
}



// DOM STUFF


function _Browser_withNode(id, doStuff)
{
	return _Scheduler_binding(function(callback)
	{
		_Browser_requestAnimationFrame(function() {
			var node = document.getElementById(id);
			callback(node
				? _Scheduler_succeed(doStuff(node))
				: _Scheduler_fail($elm$browser$Browser$Dom$NotFound(id))
			);
		});
	});
}


function _Browser_withWindow(doStuff)
{
	return _Scheduler_binding(function(callback)
	{
		_Browser_requestAnimationFrame(function() {
			callback(_Scheduler_succeed(doStuff()));
		});
	});
}


// FOCUS and BLUR


var _Browser_call = F2(function(functionName, id)
{
	return _Browser_withNode(id, function(node) {
		node[functionName]();
		return _Utils_Tuple0;
	});
});



// WINDOW VIEWPORT


function _Browser_getViewport()
{
	return {
		scene: _Browser_getScene(),
		viewport: {
			x: _Browser_window.pageXOffset,
			y: _Browser_window.pageYOffset,
			width: _Browser_doc.documentElement.clientWidth,
			height: _Browser_doc.documentElement.clientHeight
		}
	};
}

function _Browser_getScene()
{
	var body = _Browser_doc.body;
	var elem = _Browser_doc.documentElement;
	return {
		width: Math.max(body.scrollWidth, body.offsetWidth, elem.scrollWidth, elem.offsetWidth, elem.clientWidth),
		height: Math.max(body.scrollHeight, body.offsetHeight, elem.scrollHeight, elem.offsetHeight, elem.clientHeight)
	};
}

var _Browser_setViewport = F2(function(x, y)
{
	return _Browser_withWindow(function()
	{
		_Browser_window.scroll(x, y);
		return _Utils_Tuple0;
	});
});



// ELEMENT VIEWPORT


function _Browser_getViewportOf(id)
{
	return _Browser_withNode(id, function(node)
	{
		return {
			scene: {
				width: node.scrollWidth,
				height: node.scrollHeight
			},
			viewport: {
				x: node.scrollLeft,
				y: node.scrollTop,
				width: node.clientWidth,
				height: node.clientHeight
			}
		};
	});
}


var _Browser_setViewportOf = F3(function(id, x, y)
{
	return _Browser_withNode(id, function(node)
	{
		node.scrollLeft = x;
		node.scrollTop = y;
		return _Utils_Tuple0;
	});
});



// ELEMENT


function _Browser_getElement(id)
{
	return _Browser_withNode(id, function(node)
	{
		var rect = node.getBoundingClientRect();
		var x = _Browser_window.pageXOffset;
		var y = _Browser_window.pageYOffset;
		return {
			scene: _Browser_getScene(),
			viewport: {
				x: x,
				y: y,
				width: _Browser_doc.documentElement.clientWidth,
				height: _Browser_doc.documentElement.clientHeight
			},
			element: {
				x: x + rect.left,
				y: y + rect.top,
				width: rect.width,
				height: rect.height
			}
		};
	});
}



// LOAD and RELOAD


function _Browser_reload(skipCache)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function(callback)
	{
		_VirtualDom_doc.location.reload(skipCache);
	}));
}

function _Browser_load(url)
{
	return A2($elm$core$Task$perform, $elm$core$Basics$never, _Scheduler_binding(function(callback)
	{
		try
		{
			_Browser_window.location = url;
		}
		catch(err)
		{
			// Only Firefox can throw a NS_ERROR_MALFORMED_URI exception here.
			// Other browsers reload the page, so let's be consistent about that.
			_VirtualDom_doc.location.reload(false);
		}
	}));
}
var $author$project$Main$UrlChanged = function (a) {
	return {$: 'UrlChanged', a: a};
};
var $author$project$Main$UrlRequested = function (a) {
	return {$: 'UrlRequested', a: a};
};
var $elm$core$Basics$EQ = {$: 'EQ'};
var $elm$core$Basics$GT = {$: 'GT'};
var $elm$core$Basics$LT = {$: 'LT'};
var $elm$core$List$cons = _List_cons;
var $elm$core$Dict$foldr = F3(
	function (func, acc, t) {
		foldr:
		while (true) {
			if (t.$ === 'RBEmpty_elm_builtin') {
				return acc;
			} else {
				var key = t.b;
				var value = t.c;
				var left = t.d;
				var right = t.e;
				var $temp$func = func,
					$temp$acc = A3(
					func,
					key,
					value,
					A3($elm$core$Dict$foldr, func, acc, right)),
					$temp$t = left;
				func = $temp$func;
				acc = $temp$acc;
				t = $temp$t;
				continue foldr;
			}
		}
	});
var $elm$core$Dict$toList = function (dict) {
	return A3(
		$elm$core$Dict$foldr,
		F3(
			function (key, value, list) {
				return A2(
					$elm$core$List$cons,
					_Utils_Tuple2(key, value),
					list);
			}),
		_List_Nil,
		dict);
};
var $elm$core$Dict$keys = function (dict) {
	return A3(
		$elm$core$Dict$foldr,
		F3(
			function (key, value, keyList) {
				return A2($elm$core$List$cons, key, keyList);
			}),
		_List_Nil,
		dict);
};
var $elm$core$Set$toList = function (_v0) {
	var dict = _v0.a;
	return $elm$core$Dict$keys(dict);
};
var $elm$core$Elm$JsArray$foldr = _JsArray_foldr;
var $elm$core$Array$foldr = F3(
	function (func, baseCase, _v0) {
		var tree = _v0.c;
		var tail = _v0.d;
		var helper = F2(
			function (node, acc) {
				if (node.$ === 'SubTree') {
					var subTree = node.a;
					return A3($elm$core$Elm$JsArray$foldr, helper, acc, subTree);
				} else {
					var values = node.a;
					return A3($elm$core$Elm$JsArray$foldr, func, acc, values);
				}
			});
		return A3(
			$elm$core$Elm$JsArray$foldr,
			helper,
			A3($elm$core$Elm$JsArray$foldr, func, baseCase, tail),
			tree);
	});
var $elm$core$Array$toList = function (array) {
	return A3($elm$core$Array$foldr, $elm$core$List$cons, _List_Nil, array);
};
var $elm$core$Result$Err = function (a) {
	return {$: 'Err', a: a};
};
var $elm$json$Json$Decode$Failure = F2(
	function (a, b) {
		return {$: 'Failure', a: a, b: b};
	});
var $elm$json$Json$Decode$Field = F2(
	function (a, b) {
		return {$: 'Field', a: a, b: b};
	});
var $elm$json$Json$Decode$Index = F2(
	function (a, b) {
		return {$: 'Index', a: a, b: b};
	});
var $elm$core$Result$Ok = function (a) {
	return {$: 'Ok', a: a};
};
var $elm$json$Json$Decode$OneOf = function (a) {
	return {$: 'OneOf', a: a};
};
var $elm$core$Basics$False = {$: 'False'};
var $elm$core$Basics$add = _Basics_add;
var $elm$core$Maybe$Just = function (a) {
	return {$: 'Just', a: a};
};
var $elm$core$Maybe$Nothing = {$: 'Nothing'};
var $elm$core$String$all = _String_all;
var $elm$core$Basics$and = _Basics_and;
var $elm$core$Basics$append = _Utils_append;
var $elm$json$Json$Encode$encode = _Json_encode;
var $elm$core$String$fromInt = _String_fromNumber;
var $elm$core$String$join = F2(
	function (sep, chunks) {
		return A2(
			_String_join,
			sep,
			_List_toArray(chunks));
	});
var $elm$core$String$split = F2(
	function (sep, string) {
		return _List_fromArray(
			A2(_String_split, sep, string));
	});
var $elm$json$Json$Decode$indent = function (str) {
	return A2(
		$elm$core$String$join,
		'\n    ',
		A2($elm$core$String$split, '\n', str));
};
var $elm$core$List$foldl = F3(
	function (func, acc, list) {
		foldl:
		while (true) {
			if (!list.b) {
				return acc;
			} else {
				var x = list.a;
				var xs = list.b;
				var $temp$func = func,
					$temp$acc = A2(func, x, acc),
					$temp$list = xs;
				func = $temp$func;
				acc = $temp$acc;
				list = $temp$list;
				continue foldl;
			}
		}
	});
var $elm$core$List$length = function (xs) {
	return A3(
		$elm$core$List$foldl,
		F2(
			function (_v0, i) {
				return i + 1;
			}),
		0,
		xs);
};
var $elm$core$List$map2 = _List_map2;
var $elm$core$Basics$le = _Utils_le;
var $elm$core$Basics$sub = _Basics_sub;
var $elm$core$List$rangeHelp = F3(
	function (lo, hi, list) {
		rangeHelp:
		while (true) {
			if (_Utils_cmp(lo, hi) < 1) {
				var $temp$lo = lo,
					$temp$hi = hi - 1,
					$temp$list = A2($elm$core$List$cons, hi, list);
				lo = $temp$lo;
				hi = $temp$hi;
				list = $temp$list;
				continue rangeHelp;
			} else {
				return list;
			}
		}
	});
var $elm$core$List$range = F2(
	function (lo, hi) {
		return A3($elm$core$List$rangeHelp, lo, hi, _List_Nil);
	});
var $elm$core$List$indexedMap = F2(
	function (f, xs) {
		return A3(
			$elm$core$List$map2,
			f,
			A2(
				$elm$core$List$range,
				0,
				$elm$core$List$length(xs) - 1),
			xs);
	});
var $elm$core$Char$toCode = _Char_toCode;
var $elm$core$Char$isLower = function (_char) {
	var code = $elm$core$Char$toCode(_char);
	return (97 <= code) && (code <= 122);
};
var $elm$core$Char$isUpper = function (_char) {
	var code = $elm$core$Char$toCode(_char);
	return (code <= 90) && (65 <= code);
};
var $elm$core$Basics$or = _Basics_or;
var $elm$core$Char$isAlpha = function (_char) {
	return $elm$core$Char$isLower(_char) || $elm$core$Char$isUpper(_char);
};
var $elm$core$Char$isDigit = function (_char) {
	var code = $elm$core$Char$toCode(_char);
	return (code <= 57) && (48 <= code);
};
var $elm$core$Char$isAlphaNum = function (_char) {
	return $elm$core$Char$isLower(_char) || ($elm$core$Char$isUpper(_char) || $elm$core$Char$isDigit(_char));
};
var $elm$core$List$reverse = function (list) {
	return A3($elm$core$List$foldl, $elm$core$List$cons, _List_Nil, list);
};
var $elm$core$String$uncons = _String_uncons;
var $elm$json$Json$Decode$errorOneOf = F2(
	function (i, error) {
		return '\n\n(' + ($elm$core$String$fromInt(i + 1) + (') ' + $elm$json$Json$Decode$indent(
			$elm$json$Json$Decode$errorToString(error))));
	});
var $elm$json$Json$Decode$errorToString = function (error) {
	return A2($elm$json$Json$Decode$errorToStringHelp, error, _List_Nil);
};
var $elm$json$Json$Decode$errorToStringHelp = F2(
	function (error, context) {
		errorToStringHelp:
		while (true) {
			switch (error.$) {
				case 'Field':
					var f = error.a;
					var err = error.b;
					var isSimple = function () {
						var _v1 = $elm$core$String$uncons(f);
						if (_v1.$ === 'Nothing') {
							return false;
						} else {
							var _v2 = _v1.a;
							var _char = _v2.a;
							var rest = _v2.b;
							return $elm$core$Char$isAlpha(_char) && A2($elm$core$String$all, $elm$core$Char$isAlphaNum, rest);
						}
					}();
					var fieldName = isSimple ? ('.' + f) : ('[\'' + (f + '\']'));
					var $temp$error = err,
						$temp$context = A2($elm$core$List$cons, fieldName, context);
					error = $temp$error;
					context = $temp$context;
					continue errorToStringHelp;
				case 'Index':
					var i = error.a;
					var err = error.b;
					var indexName = '[' + ($elm$core$String$fromInt(i) + ']');
					var $temp$error = err,
						$temp$context = A2($elm$core$List$cons, indexName, context);
					error = $temp$error;
					context = $temp$context;
					continue errorToStringHelp;
				case 'OneOf':
					var errors = error.a;
					if (!errors.b) {
						return 'Ran into a Json.Decode.oneOf with no possibilities' + function () {
							if (!context.b) {
								return '!';
							} else {
								return ' at json' + A2(
									$elm$core$String$join,
									'',
									$elm$core$List$reverse(context));
							}
						}();
					} else {
						if (!errors.b.b) {
							var err = errors.a;
							var $temp$error = err,
								$temp$context = context;
							error = $temp$error;
							context = $temp$context;
							continue errorToStringHelp;
						} else {
							var starter = function () {
								if (!context.b) {
									return 'Json.Decode.oneOf';
								} else {
									return 'The Json.Decode.oneOf at json' + A2(
										$elm$core$String$join,
										'',
										$elm$core$List$reverse(context));
								}
							}();
							var introduction = starter + (' failed in the following ' + ($elm$core$String$fromInt(
								$elm$core$List$length(errors)) + ' ways:'));
							return A2(
								$elm$core$String$join,
								'\n\n',
								A2(
									$elm$core$List$cons,
									introduction,
									A2($elm$core$List$indexedMap, $elm$json$Json$Decode$errorOneOf, errors)));
						}
					}
				default:
					var msg = error.a;
					var json = error.b;
					var introduction = function () {
						if (!context.b) {
							return 'Problem with the given value:\n\n';
						} else {
							return 'Problem with the value at json' + (A2(
								$elm$core$String$join,
								'',
								$elm$core$List$reverse(context)) + ':\n\n    ');
						}
					}();
					return introduction + ($elm$json$Json$Decode$indent(
						A2($elm$json$Json$Encode$encode, 4, json)) + ('\n\n' + msg));
			}
		}
	});
var $elm$core$Array$branchFactor = 32;
var $elm$core$Array$Array_elm_builtin = F4(
	function (a, b, c, d) {
		return {$: 'Array_elm_builtin', a: a, b: b, c: c, d: d};
	});
var $elm$core$Elm$JsArray$empty = _JsArray_empty;
var $elm$core$Basics$ceiling = _Basics_ceiling;
var $elm$core$Basics$fdiv = _Basics_fdiv;
var $elm$core$Basics$logBase = F2(
	function (base, number) {
		return _Basics_log(number) / _Basics_log(base);
	});
var $elm$core$Basics$toFloat = _Basics_toFloat;
var $elm$core$Array$shiftStep = $elm$core$Basics$ceiling(
	A2($elm$core$Basics$logBase, 2, $elm$core$Array$branchFactor));
var $elm$core$Array$empty = A4($elm$core$Array$Array_elm_builtin, 0, $elm$core$Array$shiftStep, $elm$core$Elm$JsArray$empty, $elm$core$Elm$JsArray$empty);
var $elm$core$Elm$JsArray$initialize = _JsArray_initialize;
var $elm$core$Array$Leaf = function (a) {
	return {$: 'Leaf', a: a};
};
var $elm$core$Basics$apL = F2(
	function (f, x) {
		return f(x);
	});
var $elm$core$Basics$apR = F2(
	function (x, f) {
		return f(x);
	});
var $elm$core$Basics$eq = _Utils_equal;
var $elm$core$Basics$floor = _Basics_floor;
var $elm$core$Elm$JsArray$length = _JsArray_length;
var $elm$core$Basics$gt = _Utils_gt;
var $elm$core$Basics$max = F2(
	function (x, y) {
		return (_Utils_cmp(x, y) > 0) ? x : y;
	});
var $elm$core$Basics$mul = _Basics_mul;
var $elm$core$Array$SubTree = function (a) {
	return {$: 'SubTree', a: a};
};
var $elm$core$Elm$JsArray$initializeFromList = _JsArray_initializeFromList;
var $elm$core$Array$compressNodes = F2(
	function (nodes, acc) {
		compressNodes:
		while (true) {
			var _v0 = A2($elm$core$Elm$JsArray$initializeFromList, $elm$core$Array$branchFactor, nodes);
			var node = _v0.a;
			var remainingNodes = _v0.b;
			var newAcc = A2(
				$elm$core$List$cons,
				$elm$core$Array$SubTree(node),
				acc);
			if (!remainingNodes.b) {
				return $elm$core$List$reverse(newAcc);
			} else {
				var $temp$nodes = remainingNodes,
					$temp$acc = newAcc;
				nodes = $temp$nodes;
				acc = $temp$acc;
				continue compressNodes;
			}
		}
	});
var $elm$core$Tuple$first = function (_v0) {
	var x = _v0.a;
	return x;
};
var $elm$core$Array$treeFromBuilder = F2(
	function (nodeList, nodeListSize) {
		treeFromBuilder:
		while (true) {
			var newNodeSize = $elm$core$Basics$ceiling(nodeListSize / $elm$core$Array$branchFactor);
			if (newNodeSize === 1) {
				return A2($elm$core$Elm$JsArray$initializeFromList, $elm$core$Array$branchFactor, nodeList).a;
			} else {
				var $temp$nodeList = A2($elm$core$Array$compressNodes, nodeList, _List_Nil),
					$temp$nodeListSize = newNodeSize;
				nodeList = $temp$nodeList;
				nodeListSize = $temp$nodeListSize;
				continue treeFromBuilder;
			}
		}
	});
var $elm$core$Array$builderToArray = F2(
	function (reverseNodeList, builder) {
		if (!builder.nodeListSize) {
			return A4(
				$elm$core$Array$Array_elm_builtin,
				$elm$core$Elm$JsArray$length(builder.tail),
				$elm$core$Array$shiftStep,
				$elm$core$Elm$JsArray$empty,
				builder.tail);
		} else {
			var treeLen = builder.nodeListSize * $elm$core$Array$branchFactor;
			var depth = $elm$core$Basics$floor(
				A2($elm$core$Basics$logBase, $elm$core$Array$branchFactor, treeLen - 1));
			var correctNodeList = reverseNodeList ? $elm$core$List$reverse(builder.nodeList) : builder.nodeList;
			var tree = A2($elm$core$Array$treeFromBuilder, correctNodeList, builder.nodeListSize);
			return A4(
				$elm$core$Array$Array_elm_builtin,
				$elm$core$Elm$JsArray$length(builder.tail) + treeLen,
				A2($elm$core$Basics$max, 5, depth * $elm$core$Array$shiftStep),
				tree,
				builder.tail);
		}
	});
var $elm$core$Basics$idiv = _Basics_idiv;
var $elm$core$Basics$lt = _Utils_lt;
var $elm$core$Array$initializeHelp = F5(
	function (fn, fromIndex, len, nodeList, tail) {
		initializeHelp:
		while (true) {
			if (fromIndex < 0) {
				return A2(
					$elm$core$Array$builderToArray,
					false,
					{nodeList: nodeList, nodeListSize: (len / $elm$core$Array$branchFactor) | 0, tail: tail});
			} else {
				var leaf = $elm$core$Array$Leaf(
					A3($elm$core$Elm$JsArray$initialize, $elm$core$Array$branchFactor, fromIndex, fn));
				var $temp$fn = fn,
					$temp$fromIndex = fromIndex - $elm$core$Array$branchFactor,
					$temp$len = len,
					$temp$nodeList = A2($elm$core$List$cons, leaf, nodeList),
					$temp$tail = tail;
				fn = $temp$fn;
				fromIndex = $temp$fromIndex;
				len = $temp$len;
				nodeList = $temp$nodeList;
				tail = $temp$tail;
				continue initializeHelp;
			}
		}
	});
var $elm$core$Basics$remainderBy = _Basics_remainderBy;
var $elm$core$Array$initialize = F2(
	function (len, fn) {
		if (len <= 0) {
			return $elm$core$Array$empty;
		} else {
			var tailLen = len % $elm$core$Array$branchFactor;
			var tail = A3($elm$core$Elm$JsArray$initialize, tailLen, len - tailLen, fn);
			var initialFromIndex = (len - tailLen) - $elm$core$Array$branchFactor;
			return A5($elm$core$Array$initializeHelp, fn, initialFromIndex, len, _List_Nil, tail);
		}
	});
var $elm$core$Basics$True = {$: 'True'};
var $elm$core$Result$isOk = function (result) {
	if (result.$ === 'Ok') {
		return true;
	} else {
		return false;
	}
};
var $elm$json$Json$Decode$map = _Json_map1;
var $elm$json$Json$Decode$map2 = _Json_map2;
var $elm$json$Json$Decode$succeed = _Json_succeed;
var $elm$virtual_dom$VirtualDom$toHandlerInt = function (handler) {
	switch (handler.$) {
		case 'Normal':
			return 0;
		case 'MayStopPropagation':
			return 1;
		case 'MayPreventDefault':
			return 2;
		default:
			return 3;
	}
};
var $elm$browser$Browser$External = function (a) {
	return {$: 'External', a: a};
};
var $elm$browser$Browser$Internal = function (a) {
	return {$: 'Internal', a: a};
};
var $elm$core$Basics$identity = function (x) {
	return x;
};
var $elm$browser$Browser$Dom$NotFound = function (a) {
	return {$: 'NotFound', a: a};
};
var $elm$url$Url$Http = {$: 'Http'};
var $elm$url$Url$Https = {$: 'Https'};
var $elm$url$Url$Url = F6(
	function (protocol, host, port_, path, query, fragment) {
		return {fragment: fragment, host: host, path: path, port_: port_, protocol: protocol, query: query};
	});
var $elm$core$String$contains = _String_contains;
var $elm$core$String$length = _String_length;
var $elm$core$String$slice = _String_slice;
var $elm$core$String$dropLeft = F2(
	function (n, string) {
		return (n < 1) ? string : A3(
			$elm$core$String$slice,
			n,
			$elm$core$String$length(string),
			string);
	});
var $elm$core$String$indexes = _String_indexes;
var $elm$core$String$isEmpty = function (string) {
	return string === '';
};
var $elm$core$String$left = F2(
	function (n, string) {
		return (n < 1) ? '' : A3($elm$core$String$slice, 0, n, string);
	});
var $elm$core$String$toInt = _String_toInt;
var $elm$url$Url$chompBeforePath = F5(
	function (protocol, path, params, frag, str) {
		if ($elm$core$String$isEmpty(str) || A2($elm$core$String$contains, '@', str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, ':', str);
			if (!_v0.b) {
				return $elm$core$Maybe$Just(
					A6($elm$url$Url$Url, protocol, str, $elm$core$Maybe$Nothing, path, params, frag));
			} else {
				if (!_v0.b.b) {
					var i = _v0.a;
					var _v1 = $elm$core$String$toInt(
						A2($elm$core$String$dropLeft, i + 1, str));
					if (_v1.$ === 'Nothing') {
						return $elm$core$Maybe$Nothing;
					} else {
						var port_ = _v1;
						return $elm$core$Maybe$Just(
							A6(
								$elm$url$Url$Url,
								protocol,
								A2($elm$core$String$left, i, str),
								port_,
								path,
								params,
								frag));
					}
				} else {
					return $elm$core$Maybe$Nothing;
				}
			}
		}
	});
var $elm$url$Url$chompBeforeQuery = F4(
	function (protocol, params, frag, str) {
		if ($elm$core$String$isEmpty(str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, '/', str);
			if (!_v0.b) {
				return A5($elm$url$Url$chompBeforePath, protocol, '/', params, frag, str);
			} else {
				var i = _v0.a;
				return A5(
					$elm$url$Url$chompBeforePath,
					protocol,
					A2($elm$core$String$dropLeft, i, str),
					params,
					frag,
					A2($elm$core$String$left, i, str));
			}
		}
	});
var $elm$url$Url$chompBeforeFragment = F3(
	function (protocol, frag, str) {
		if ($elm$core$String$isEmpty(str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, '?', str);
			if (!_v0.b) {
				return A4($elm$url$Url$chompBeforeQuery, protocol, $elm$core$Maybe$Nothing, frag, str);
			} else {
				var i = _v0.a;
				return A4(
					$elm$url$Url$chompBeforeQuery,
					protocol,
					$elm$core$Maybe$Just(
						A2($elm$core$String$dropLeft, i + 1, str)),
					frag,
					A2($elm$core$String$left, i, str));
			}
		}
	});
var $elm$url$Url$chompAfterProtocol = F2(
	function (protocol, str) {
		if ($elm$core$String$isEmpty(str)) {
			return $elm$core$Maybe$Nothing;
		} else {
			var _v0 = A2($elm$core$String$indexes, '#', str);
			if (!_v0.b) {
				return A3($elm$url$Url$chompBeforeFragment, protocol, $elm$core$Maybe$Nothing, str);
			} else {
				var i = _v0.a;
				return A3(
					$elm$url$Url$chompBeforeFragment,
					protocol,
					$elm$core$Maybe$Just(
						A2($elm$core$String$dropLeft, i + 1, str)),
					A2($elm$core$String$left, i, str));
			}
		}
	});
var $elm$core$String$startsWith = _String_startsWith;
var $elm$url$Url$fromString = function (str) {
	return A2($elm$core$String$startsWith, 'http://', str) ? A2(
		$elm$url$Url$chompAfterProtocol,
		$elm$url$Url$Http,
		A2($elm$core$String$dropLeft, 7, str)) : (A2($elm$core$String$startsWith, 'https://', str) ? A2(
		$elm$url$Url$chompAfterProtocol,
		$elm$url$Url$Https,
		A2($elm$core$String$dropLeft, 8, str)) : $elm$core$Maybe$Nothing);
};
var $elm$core$Basics$never = function (_v0) {
	never:
	while (true) {
		var nvr = _v0.a;
		var $temp$_v0 = nvr;
		_v0 = $temp$_v0;
		continue never;
	}
};
var $elm$core$Task$Perform = function (a) {
	return {$: 'Perform', a: a};
};
var $elm$core$Task$succeed = _Scheduler_succeed;
var $elm$core$Task$init = $elm$core$Task$succeed(_Utils_Tuple0);
var $elm$core$List$foldrHelper = F4(
	function (fn, acc, ctr, ls) {
		if (!ls.b) {
			return acc;
		} else {
			var a = ls.a;
			var r1 = ls.b;
			if (!r1.b) {
				return A2(fn, a, acc);
			} else {
				var b = r1.a;
				var r2 = r1.b;
				if (!r2.b) {
					return A2(
						fn,
						a,
						A2(fn, b, acc));
				} else {
					var c = r2.a;
					var r3 = r2.b;
					if (!r3.b) {
						return A2(
							fn,
							a,
							A2(
								fn,
								b,
								A2(fn, c, acc)));
					} else {
						var d = r3.a;
						var r4 = r3.b;
						var res = (ctr > 500) ? A3(
							$elm$core$List$foldl,
							fn,
							acc,
							$elm$core$List$reverse(r4)) : A4($elm$core$List$foldrHelper, fn, acc, ctr + 1, r4);
						return A2(
							fn,
							a,
							A2(
								fn,
								b,
								A2(
									fn,
									c,
									A2(fn, d, res))));
					}
				}
			}
		}
	});
var $elm$core$List$foldr = F3(
	function (fn, acc, ls) {
		return A4($elm$core$List$foldrHelper, fn, acc, 0, ls);
	});
var $elm$core$List$map = F2(
	function (f, xs) {
		return A3(
			$elm$core$List$foldr,
			F2(
				function (x, acc) {
					return A2(
						$elm$core$List$cons,
						f(x),
						acc);
				}),
			_List_Nil,
			xs);
	});
var $elm$core$Task$andThen = _Scheduler_andThen;
var $elm$core$Task$map = F2(
	function (func, taskA) {
		return A2(
			$elm$core$Task$andThen,
			function (a) {
				return $elm$core$Task$succeed(
					func(a));
			},
			taskA);
	});
var $elm$core$Task$map2 = F3(
	function (func, taskA, taskB) {
		return A2(
			$elm$core$Task$andThen,
			function (a) {
				return A2(
					$elm$core$Task$andThen,
					function (b) {
						return $elm$core$Task$succeed(
							A2(func, a, b));
					},
					taskB);
			},
			taskA);
	});
var $elm$core$Task$sequence = function (tasks) {
	return A3(
		$elm$core$List$foldr,
		$elm$core$Task$map2($elm$core$List$cons),
		$elm$core$Task$succeed(_List_Nil),
		tasks);
};
var $elm$core$Platform$sendToApp = _Platform_sendToApp;
var $elm$core$Task$spawnCmd = F2(
	function (router, _v0) {
		var task = _v0.a;
		return _Scheduler_spawn(
			A2(
				$elm$core$Task$andThen,
				$elm$core$Platform$sendToApp(router),
				task));
	});
var $elm$core$Task$onEffects = F3(
	function (router, commands, state) {
		return A2(
			$elm$core$Task$map,
			function (_v0) {
				return _Utils_Tuple0;
			},
			$elm$core$Task$sequence(
				A2(
					$elm$core$List$map,
					$elm$core$Task$spawnCmd(router),
					commands)));
	});
var $elm$core$Task$onSelfMsg = F3(
	function (_v0, _v1, _v2) {
		return $elm$core$Task$succeed(_Utils_Tuple0);
	});
var $elm$core$Task$cmdMap = F2(
	function (tagger, _v0) {
		var task = _v0.a;
		return $elm$core$Task$Perform(
			A2($elm$core$Task$map, tagger, task));
	});
_Platform_effectManagers['Task'] = _Platform_createManager($elm$core$Task$init, $elm$core$Task$onEffects, $elm$core$Task$onSelfMsg, $elm$core$Task$cmdMap);
var $elm$core$Task$command = _Platform_leaf('Task');
var $elm$core$Task$perform = F2(
	function (toMessage, task) {
		return $elm$core$Task$command(
			$elm$core$Task$Perform(
				A2($elm$core$Task$map, toMessage, task)));
	});
var $elm$browser$Browser$application = _Browser_application;
var $author$project$Main$ResetPassword = {$: 'ResetPassword'};
var $elm$core$List$maybeCons = F3(
	function (f, mx, xs) {
		var _v0 = f(mx);
		if (_v0.$ === 'Just') {
			var x = _v0.a;
			return A2($elm$core$List$cons, x, xs);
		} else {
			return xs;
		}
	});
var $elm$core$List$filterMap = F2(
	function (f, xs) {
		return A3(
			$elm$core$List$foldr,
			$elm$core$List$maybeCons(f),
			_List_Nil,
			xs);
	});
var $elm$core$List$head = function (list) {
	if (list.b) {
		var x = list.a;
		var xs = list.b;
		return $elm$core$Maybe$Just(x);
	} else {
		return $elm$core$Maybe$Nothing;
	}
};
var $author$project$Main$extractResetToken = function (query) {
	return $elm$core$List$head(
		A2(
			$elm$core$List$filterMap,
			function (param) {
				var _v0 = A2($elm$core$String$split, '=', param);
				if (((_v0.b && (_v0.a === 'reset_token')) && _v0.b.b) && (!_v0.b.b.b)) {
					var _v1 = _v0.b;
					var value = _v1.a;
					return $elm$core$Maybe$Just(value);
				} else {
					return $elm$core$Maybe$Nothing;
				}
			},
			A2($elm$core$String$split, '&', query)));
};
var $author$project$Main$LoginView = {$: 'LoginView'};
var $author$project$Main$Welcome = {$: 'Welcome'};
var $author$project$Main$defaultGameModes = _List_fromArray(
	[
		{description: 'Affrontez l\'IA Graph Transformer (149 pts)', difficulty: $elm$core$Maybe$Nothing, icon: '🤖', id: 'single-player', name: 'Solo', simulations: $elm$core$Maybe$Nothing},
		{description: 'Jouez avec le vrai jeu - sélectionnez les tuiles tirées', difficulty: $elm$core$Maybe$Nothing, icon: '🎲', id: 'real-game', name: 'Jeu Réel', simulations: $elm$core$Maybe$Nothing},
		{description: 'Jouez contre d\'autres joueurs en ligne', difficulty: $elm$core$Maybe$Nothing, icon: '👥', id: 'multiplayer', name: 'Multijoueur', simulations: $elm$core$Maybe$Nothing}
	]);
var $elm$core$List$repeatHelp = F3(
	function (result, n, value) {
		repeatHelp:
		while (true) {
			if (n <= 0) {
				return result;
			} else {
				var $temp$result = A2($elm$core$List$cons, value, result),
					$temp$n = n - 1,
					$temp$value = value;
				result = $temp$result;
				n = $temp$n;
				value = $temp$value;
				continue repeatHelp;
			}
		}
	});
var $elm$core$List$repeat = F2(
	function (n, value) {
		return A3($elm$core$List$repeatHelp, _List_Nil, n, value);
	});
var $author$project$Main$initialModel = F2(
	function (key, url) {
		return {
			aiPlateauTiles: A2($elm$core$List$repeat, 19, ''),
			aiScore: 0,
			allPlayerPlateaus: _List_Nil,
			authError: '',
			authLoading: false,
			authView: $author$project$Main$Welcome,
			availablePositions: A2($elm$core$List$range, 0, 18),
			confirmPasswordInput: '',
			currentTile: $elm$core$Maybe$Nothing,
			currentTileImage: $elm$core$Maybe$Nothing,
			currentTurnNumber: 0,
			currentView: $author$project$Main$LoginView,
			emailInput: '',
			error: '',
			gameModes: $author$project$Main$defaultGameModes,
			gameState: $elm$core$Maybe$Nothing,
			isAuthenticated: false,
			isRealGameMode: false,
			isSoloMode: false,
			key: key,
			loading: false,
			myTurn: false,
			passwordInput: '',
			pendingAiPosition: $elm$core$Maybe$Nothing,
			plateauTiles: A2($elm$core$List$repeat, 19, ''),
			playerName: '',
			realGameScore: 0,
			resetMessage: '',
			resetToken: '',
			selectedGameMode: $elm$core$Maybe$Nothing,
			session: $elm$core$Maybe$Nothing,
			sessionCode: '',
			showAiBoard: false,
			showTilePicker: false,
			statusMessage: '',
			token: $elm$core$Maybe$Nothing,
			url: url,
			usedTiles: _List_Nil,
			user: $elm$core$Maybe$Nothing,
			usernameInput: '',
			waitingForPlayers: _List_Nil
		};
	});
var $elm$json$Json$Encode$object = function (pairs) {
	return _Json_wrap(
		A3(
			$elm$core$List$foldl,
			F2(
				function (_v0, obj) {
					var k = _v0.a;
					var v = _v0.b;
					return A3(_Json_addField, k, v, obj);
				}),
			_Json_emptyObject(_Utils_Tuple0),
			pairs));
};
var $author$project$Main$sendToJs = _Platform_outgoingPort('sendToJs', $elm$core$Basics$identity);
var $elm$json$Json$Encode$string = _Json_wrap;
var $author$project$Main$init = F3(
	function (_v0, url, key) {
		var baseModel = A2($author$project$Main$initialModel, key, url);
		var modelWithResetToken = function () {
			var _v1 = url.query;
			if (_v1.$ === 'Just') {
				var query = _v1.a;
				var _v2 = $author$project$Main$extractResetToken(query);
				if (_v2.$ === 'Just') {
					var token = _v2.a;
					return _Utils_update(
						baseModel,
						{authView: $author$project$Main$ResetPassword, resetToken: token});
				} else {
					return baseModel;
				}
			} else {
				return baseModel;
			}
		}();
		return _Utils_Tuple2(
			modelWithResetToken,
			$author$project$Main$sendToJs(
				$elm$json$Json$Encode$object(
					_List_fromArray(
						[
							_Utils_Tuple2(
							'type',
							$elm$json$Json$Encode$string('checkAuth'))
						]))));
	});
var $author$project$Main$ReceivedFromJs = function (a) {
	return {$: 'ReceivedFromJs', a: a};
};
var $elm$json$Json$Decode$value = _Json_decodeValue;
var $author$project$Main$receiveFromJs = _Platform_incomingPort('receiveFromJs', $elm$json$Json$Decode$value);
var $author$project$Main$subscriptions = function (_v0) {
	return $author$project$Main$receiveFromJs($author$project$Main$ReceivedFromJs);
};
var $author$project$Main$AiMoveResult = F2(
	function (a, b) {
		return {$: 'AiMoveResult', a: a, b: b};
	});
var $author$project$Main$CheckAuthFailure = {$: 'CheckAuthFailure'};
var $author$project$Main$CheckAuthSuccess = F2(
	function (a, b) {
		return {$: 'CheckAuthSuccess', a: a, b: b};
	});
var $author$project$Main$Finished = {$: 'Finished'};
var $author$project$Main$ForgotPasswordFailure = function (a) {
	return {$: 'ForgotPasswordFailure', a: a};
};
var $author$project$Main$ForgotPasswordSuccess = function (a) {
	return {$: 'ForgotPasswordSuccess', a: a};
};
var $author$project$Main$GameError = function (a) {
	return {$: 'GameError', a: a};
};
var $author$project$Main$GameFinished = F4(
	function (a, b, c, d) {
		return {$: 'GameFinished', a: a, b: b, c: c, d: d};
	});
var $author$project$Main$GameStateUpdated = function (a) {
	return {$: 'GameStateUpdated', a: a};
};
var $author$project$Main$GameView = {$: 'GameView'};
var $author$project$Main$InProgress = {$: 'InProgress'};
var $author$project$Main$Login = {$: 'Login'};
var $author$project$Main$LoginFailure = function (a) {
	return {$: 'LoginFailure', a: a};
};
var $author$project$Main$LoginSuccess = F2(
	function (a, b) {
		return {$: 'LoginSuccess', a: a, b: b};
	});
var $author$project$Main$ModeSelectionView = {$: 'ModeSelectionView'};
var $author$project$Main$MovePlayed = F5(
	function (a, b, c, d, e) {
		return {$: 'MovePlayed', a: a, b: b, c: c, d: d, e: e};
	});
var $author$project$Main$PollSession = {$: 'PollSession'};
var $author$project$Main$PollTurn = {$: 'PollTurn'};
var $author$project$Main$ReadySet = function (a) {
	return {$: 'ReadySet', a: a};
};
var $author$project$Main$RegisterFailure = function (a) {
	return {$: 'RegisterFailure', a: a};
};
var $author$project$Main$RegisterSuccess = F2(
	function (a, b) {
		return {$: 'RegisterSuccess', a: a, b: b};
	});
var $author$project$Main$ResetPasswordFailure = function (a) {
	return {$: 'ResetPasswordFailure', a: a};
};
var $author$project$Main$ResetPasswordSuccess = function (a) {
	return {$: 'ResetPasswordSuccess', a: a};
};
var $author$project$Main$SessionCreated = F2(
	function (a, b) {
		return {$: 'SessionCreated', a: a, b: b};
	});
var $author$project$Main$SessionError = function (a) {
	return {$: 'SessionError', a: a};
};
var $author$project$Main$SessionJoined = F2(
	function (a, b) {
		return {$: 'SessionJoined', a: a, b: b};
	});
var $author$project$Main$SessionLeft = {$: 'SessionLeft'};
var $author$project$Main$SessionPolled = function (a) {
	return {$: 'SessionPolled', a: a};
};
var $author$project$Main$TurnStarted = F6(
	function (a, b, c, d, e, f) {
		return {$: 'TurnStarted', a: a, b: b, c: c, d: d, e: e, f: f};
	});
var $elm$core$Platform$Cmd$batch = _Platform_batch;
var $elm$json$Json$Decode$decodeValue = _Json_run;
var $elm$core$List$filter = F2(
	function (isGood, list) {
		return A3(
			$elm$core$List$foldr,
			F2(
				function (x, xs) {
					return isGood(x) ? A2($elm$core$List$cons, x, xs) : xs;
				}),
			_List_Nil,
			list);
	});
var $elm$core$Basics$ge = _Utils_ge;
var $elm$core$Basics$neq = _Utils_notEqual;
var $author$project$GameLogic$handleAiMoveResultPure = F2(
	function (position, errorMsg) {
		return ((position >= 0) && (position < 19)) ? {
			pendingAiPosition: $elm$core$Maybe$Just(position),
			statusMessage: (errorMsg !== '') ? ('IA: ' + errorMsg) : ''
		} : {pendingAiPosition: $elm$core$Maybe$Nothing, statusMessage: 'IA: position invalide'};
	});
var $elm$core$Maybe$map = F2(
	function (f, maybe) {
		if (maybe.$ === 'Just') {
			var value = maybe.a;
			return $elm$core$Maybe$Just(
				f(value));
		} else {
			return $elm$core$Maybe$Nothing;
		}
	});
var $elm$core$Maybe$withDefault = F2(
	function (_default, maybe) {
		if (maybe.$ === 'Just') {
			var value = maybe.a;
			return value;
		} else {
			return _default;
		}
	});
var $author$project$GameLogic$handleGameFinishedPure = function (input) {
	var resolvedPlateaus = A2(
		$elm$core$List$map,
		function (_v0) {
			var id = _v0.a;
			var tiles = _v0.b;
			var name = A2(
				$elm$core$Maybe$withDefault,
				(id === 'mcts_ai') ? 'IA' : 'Joueur',
				A2(
					$elm$core$Maybe$map,
					function ($) {
						return $.name;
					},
					$elm$core$List$head(
						A2(
							$elm$core$List$filter,
							function (p) {
								return _Utils_eq(p.id, id);
							},
							input.players))));
			return _Utils_Tuple3(id, name, tiles);
		},
		input.allPlateaus);
	return {aiPlateauTiles: input.aiTiles, allPlayerPlateaus: resolvedPlateaus, gameStateIsFinished: true, myTurn: false, plateauTiles: input.playerTiles, statusMessage: 'Partie terminée!', waitingForPlayers: _List_Nil};
};
var $author$project$GameLogic$BatchCmds = function (a) {
	return {$: 'BatchCmds', a: a};
};
var $author$project$GameLogic$NoCmd = {$: 'NoCmd'};
var $author$project$GameLogic$SchedulePollTurn = function (a) {
	return {$: 'SchedulePollTurn', a: a};
};
var $author$project$GameLogic$SendStartTurn = function (a) {
	return {$: 'SendStartTurn', a: a};
};
var $elm$core$List$isEmpty = function (xs) {
	if (!xs.b) {
		return true;
	} else {
		return false;
	}
};
var $elm$core$String$replace = F3(
	function (before, after, string) {
		return A2(
			$elm$core$String$join,
			after,
			A2($elm$core$String$split, before, string));
	});
var $author$project$GameLogic$handleMovePlayedPure = F2(
	function (model, input) {
		var newPlateauTiles = A2(
			$elm$core$List$indexedMap,
			F2(
				function (i, tile) {
					return _Utils_eq(i, input.position) ? A2(
						$elm$core$Maybe$withDefault,
						tile,
						A2(
							$elm$core$Maybe$map,
							A2($elm$core$String$replace, '../', ''),
							model.currentTileImage)) : tile;
				}),
			model.plateauTiles);
		var newAvailablePositions = A2(
			$elm$core$List$filter,
			function (p) {
				return !_Utils_eq(p, input.position);
			},
			model.availablePositions);
		var newAiPlateauTiles = $elm$core$List$isEmpty(input.aiTiles) ? model.aiPlateauTiles : input.aiTiles;
		var cmdIntent = function () {
			if (input.isGameOver) {
				return $author$project$GameLogic$NoCmd;
			} else {
				var _v0 = model.sessionId;
				if (_v0.$ === 'Just') {
					var sid = _v0.a;
					return $author$project$GameLogic$BatchCmds(
						_List_fromArray(
							[
								$author$project$GameLogic$SendStartTurn(sid),
								$author$project$GameLogic$SchedulePollTurn(3000)
							]));
				} else {
					return $author$project$GameLogic$NoCmd;
				}
			}
		}();
		return {
			aiPlateauTiles: newAiPlateauTiles,
			availablePositions: newAvailablePositions,
			cmdIntent: cmdIntent,
			currentTile: $elm$core$Maybe$Nothing,
			currentTileImage: $elm$core$Maybe$Nothing,
			loading: false,
			myTurn: false,
			plateauTiles: newPlateauTiles,
			statusMessage: '+' + ($elm$core$String$fromInt(input.points) + ' points')
		};
	});
var $elm$core$Basics$not = _Basics_not;
var $author$project$GameLogic$handlePlaceRealTilePure = F2(
	function (model, position) {
		var tileImage = A2($elm$core$Maybe$withDefault, '', model.currentTileImage);
		var newTurnNumber = model.currentTurnNumber + 1;
		var newPlateauTiles = A2(
			$elm$core$List$indexedMap,
			F2(
				function (i, tile) {
					return _Utils_eq(i, position) ? tileImage : tile;
				}),
			model.plateauTiles);
		var newAvailablePositions = A2(
			$elm$core$List$filter,
			function (p) {
				return !_Utils_eq(p, position);
			},
			model.availablePositions);
		var newAiPlateauTiles = function () {
			var _v1 = model.pendingAiPosition;
			if (_v1.$ === 'Just') {
				var aiPos = _v1.a;
				return A2(
					$elm$core$List$indexedMap,
					F2(
						function (i, tile) {
							return _Utils_eq(i, aiPos) ? tileImage : tile;
						}),
					model.aiPlateauTiles);
			} else {
				return model.aiPlateauTiles;
			}
		}();
		var isGameOver = newTurnNumber >= 19;
		var aiMessage = function () {
			var _v0 = model.pendingAiPosition;
			if (_v0.$ === 'Just') {
				var aiPos = _v0.a;
				return 'IA joue en position ' + $elm$core$String$fromInt(aiPos);
			} else {
				return '';
			}
		}();
		return {
			aiPlateauTiles: newAiPlateauTiles,
			availablePositions: newAvailablePositions,
			currentTile: $elm$core$Maybe$Nothing,
			currentTileImage: $elm$core$Maybe$Nothing,
			currentTurnNumber: newTurnNumber,
			pendingAiPosition: $elm$core$Maybe$Nothing,
			plateauTiles: newPlateauTiles,
			showTilePicker: !isGameOver,
			statusMessage: isGameOver ? 'Partie terminée! Calculez votre score.' : aiMessage
		};
	});
var $author$project$GameLogic$handlePollTurnPure = function (model) {
	var _v0 = model.sessionId;
	if (_v0.$ === 'Just') {
		var sid = _v0.a;
		return (!model.myTurn) ? $author$project$GameLogic$SendStartTurn(sid) : $author$project$GameLogic$NoCmd;
	} else {
		return $author$project$GameLogic$NoCmd;
	}
};
var $author$project$GameLogic$SendGetAiMove = F4(
	function (a, b, c, d) {
		return {$: 'SendGetAiMove', a: a, b: b, c: c, d: d};
	});
var $author$project$GameLogic$handleSelectRealTilePure = F2(
	function (model, tileCode) {
		var aiAvailablePositions = A2(
			$elm$core$List$map,
			$elm$core$Tuple$first,
			A2(
				$elm$core$List$filter,
				function (_v0) {
					var tile = _v0.b;
					return tile === '';
				},
				A2(
					$elm$core$List$indexedMap,
					F2(
						function (i, tile) {
							return _Utils_Tuple2(i, tile);
						}),
					model.aiPlateauTiles)));
		return {
			cmdIntent: A4($author$project$GameLogic$SendGetAiMove, tileCode, model.aiPlateauTiles, aiAvailablePositions, model.currentTurnNumber),
			currentTile: $elm$core$Maybe$Just(tileCode),
			currentTileImage: $elm$core$Maybe$Just('image/' + (tileCode + '.png')),
			showTilePicker: false,
			usedTiles: A2($elm$core$List$cons, tileCode, model.usedTiles)
		};
	});
var $elm$core$List$any = F2(
	function (isOkay, list) {
		any:
		while (true) {
			if (!list.b) {
				return false;
			} else {
				var x = list.a;
				var xs = list.b;
				if (isOkay(x)) {
					return true;
				} else {
					var $temp$isOkay = isOkay,
						$temp$list = xs;
					isOkay = $temp$isOkay;
					list = $temp$list;
					continue any;
				}
			}
		}
	});
var $elm$core$List$member = F2(
	function (x, xs) {
		return A2(
			$elm$core$List$any,
			function (a) {
				return _Utils_eq(a, x);
			},
			xs);
	});
var $author$project$GameLogic$handleTurnStartedPure = F2(
	function (model, input) {
		var playerId = A2($elm$core$Maybe$withDefault, '', model.playerId);
		var isMyTurn = A2($elm$core$List$member, playerId, input.waiting);
		var pollCmd = (!isMyTurn) ? $author$project$GameLogic$SchedulePollTurn(2000) : $author$project$GameLogic$NoCmd;
		return {
			availablePositions: input.positions,
			cmdIntent: pollCmd,
			currentTile: isMyTurn ? $elm$core$Maybe$Just(input.tile) : $elm$core$Maybe$Nothing,
			currentTileImage: isMyTurn ? $elm$core$Maybe$Just(input.tileImage) : $elm$core$Maybe$Nothing,
			currentTurnNumber: input.turnNumber,
			loading: false,
			myTurn: isMyTurn,
			waitingForPlayers: input.waiting
		};
	});
var $elm$json$Json$Encode$int = _Json_wrap;
var $elm$json$Json$Decode$andThen = _Json_andThen;
var $elm$json$Json$Decode$field = _Json_decodeField;
var $author$project$Main$JsAiMoveResult = F2(
	function (a, b) {
		return {$: 'JsAiMoveResult', a: a, b: b};
	});
var $author$project$Main$JsCheckAuthFailure = {$: 'JsCheckAuthFailure'};
var $author$project$Main$JsCheckAuthSuccess = F2(
	function (a, b) {
		return {$: 'JsCheckAuthSuccess', a: a, b: b};
	});
var $author$project$Main$JsForgotPasswordFailure = function (a) {
	return {$: 'JsForgotPasswordFailure', a: a};
};
var $author$project$Main$JsForgotPasswordSuccess = function (a) {
	return {$: 'JsForgotPasswordSuccess', a: a};
};
var $author$project$Main$JsGameError = function (a) {
	return {$: 'JsGameError', a: a};
};
var $author$project$Main$JsGameFinished = F4(
	function (a, b, c, d) {
		return {$: 'JsGameFinished', a: a, b: b, c: c, d: d};
	});
var $author$project$Main$JsGameStateUpdated = function (a) {
	return {$: 'JsGameStateUpdated', a: a};
};
var $author$project$Main$JsLoginFailure = function (a) {
	return {$: 'JsLoginFailure', a: a};
};
var $author$project$Main$JsLoginSuccess = F2(
	function (a, b) {
		return {$: 'JsLoginSuccess', a: a, b: b};
	});
var $author$project$Main$JsMovePlayed = F5(
	function (a, b, c, d, e) {
		return {$: 'JsMovePlayed', a: a, b: b, c: c, d: d, e: e};
	});
var $author$project$Main$JsReadySet = function (a) {
	return {$: 'JsReadySet', a: a};
};
var $author$project$Main$JsRegisterFailure = function (a) {
	return {$: 'JsRegisterFailure', a: a};
};
var $author$project$Main$JsRegisterSuccess = F2(
	function (a, b) {
		return {$: 'JsRegisterSuccess', a: a, b: b};
	});
var $author$project$Main$JsResetPasswordFailure = function (a) {
	return {$: 'JsResetPasswordFailure', a: a};
};
var $author$project$Main$JsResetPasswordSuccess = function (a) {
	return {$: 'JsResetPasswordSuccess', a: a};
};
var $author$project$Main$JsSessionCreated = F2(
	function (a, b) {
		return {$: 'JsSessionCreated', a: a, b: b};
	});
var $author$project$Main$JsSessionError = function (a) {
	return {$: 'JsSessionError', a: a};
};
var $author$project$Main$JsSessionJoined = F2(
	function (a, b) {
		return {$: 'JsSessionJoined', a: a, b: b};
	});
var $author$project$Main$JsSessionLeft = {$: 'JsSessionLeft'};
var $author$project$Main$JsSessionPolled = function (a) {
	return {$: 'JsSessionPolled', a: a};
};
var $author$project$Main$JsTurnStarted = F6(
	function (a, b, c, d, e, f) {
		return {$: 'JsTurnStarted', a: a, b: b, c: c, d: d, e: e, f: f};
	});
var $elm$json$Json$Decode$at = F2(
	function (fields, decoder) {
		return A3($elm$core$List$foldr, $elm$json$Json$Decode$field, decoder, fields);
	});
var $elm$json$Json$Decode$bool = _Json_decodeBool;
var $elm$json$Json$Decode$fail = _Json_fail;
var $author$project$Main$GameState = F4(
	function (sessionCode, state, players, currentTurn) {
		return {currentTurn: currentTurn, players: players, sessionCode: sessionCode, state: state};
	});
var $elm$json$Json$Decode$list = _Json_decodeList;
var $elm$json$Json$Decode$map4 = _Json_map4;
var $elm$json$Json$Decode$oneOf = _Json_oneOf;
var $elm$json$Json$Decode$maybe = function (decoder) {
	return $elm$json$Json$Decode$oneOf(
		_List_fromArray(
			[
				A2($elm$json$Json$Decode$map, $elm$core$Maybe$Just, decoder),
				$elm$json$Json$Decode$succeed($elm$core$Maybe$Nothing)
			]));
};
var $author$project$Main$Player = F5(
	function (id, name, score, isReady, isConnected) {
		return {id: id, isConnected: isConnected, isReady: isReady, name: name, score: score};
	});
var $elm$json$Json$Decode$int = _Json_decodeInt;
var $elm$json$Json$Decode$map5 = _Json_map5;
var $elm$json$Json$Decode$string = _Json_decodeString;
var $author$project$Main$playerDecoder = A6(
	$elm$json$Json$Decode$map5,
	$author$project$Main$Player,
	A2($elm$json$Json$Decode$field, 'id', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'name', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'score', $elm$json$Json$Decode$int),
	A2($elm$json$Json$Decode$field, 'isReady', $elm$json$Json$Decode$bool),
	A2($elm$json$Json$Decode$field, 'isConnected', $elm$json$Json$Decode$bool));
var $author$project$Main$Cancelled = {$: 'Cancelled'};
var $author$project$Main$Waiting = {$: 'Waiting'};
var $author$project$Main$sessionStateDecoder = A2(
	$elm$json$Json$Decode$andThen,
	function (n) {
		switch (n) {
			case 0:
				return $elm$json$Json$Decode$succeed($author$project$Main$Waiting);
			case 1:
				return $elm$json$Json$Decode$succeed($author$project$Main$InProgress);
			case 2:
				return $elm$json$Json$Decode$succeed($author$project$Main$Finished);
			case 3:
				return $elm$json$Json$Decode$succeed($author$project$Main$Cancelled);
			default:
				return $elm$json$Json$Decode$succeed($author$project$Main$Waiting);
		}
	},
	$elm$json$Json$Decode$int);
var $author$project$Main$gameStateDecoder = A5(
	$elm$json$Json$Decode$map4,
	$author$project$Main$GameState,
	A2($elm$json$Json$Decode$field, 'sessionCode', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'state', $author$project$Main$sessionStateDecoder),
	A2(
		$elm$json$Json$Decode$field,
		'players',
		$elm$json$Json$Decode$list($author$project$Main$playerDecoder)),
	$elm$json$Json$Decode$maybe(
		A2($elm$json$Json$Decode$field, 'currentTurn', $elm$json$Json$Decode$string)));
var $elm$json$Json$Decode$keyValuePairs = _Json_decodeKeyValuePairs;
var $elm$json$Json$Decode$map6 = _Json_map6;
var $elm$core$Tuple$second = function (_v0) {
	var y = _v0.b;
	return y;
};
var $author$project$Main$Session = F3(
	function (sessionId, playerId, sessionCode) {
		return {playerId: playerId, sessionCode: sessionCode, sessionId: sessionId};
	});
var $elm$json$Json$Decode$map3 = _Json_map3;
var $author$project$Main$sessionDecoder = A4(
	$elm$json$Json$Decode$map3,
	$author$project$Main$Session,
	A2($elm$json$Json$Decode$field, 'sessionId', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'playerId', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'sessionCode', $elm$json$Json$Decode$string));
var $author$project$Main$User = F4(
	function (id, email, username, emailVerified) {
		return {email: email, emailVerified: emailVerified, id: id, username: username};
	});
var $author$project$Main$userDecoder = A5(
	$elm$json$Json$Decode$map4,
	$author$project$Main$User,
	A2($elm$json$Json$Decode$field, 'id', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'email', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'username', $elm$json$Json$Decode$string),
	A2($elm$json$Json$Decode$field, 'emailVerified', $elm$json$Json$Decode$bool));
var $author$project$Main$jsMessageDecoderByType = function (msgType) {
	switch (msgType) {
		case 'loginSuccess':
			return A3(
				$elm$json$Json$Decode$map2,
				$author$project$Main$JsLoginSuccess,
				A2($elm$json$Json$Decode$field, 'user', $author$project$Main$userDecoder),
				A2($elm$json$Json$Decode$field, 'token', $elm$json$Json$Decode$string));
		case 'loginFailure':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsLoginFailure,
				A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string));
		case 'registerSuccess':
			return A3(
				$elm$json$Json$Decode$map2,
				$author$project$Main$JsRegisterSuccess,
				A2($elm$json$Json$Decode$field, 'user', $author$project$Main$userDecoder),
				A2($elm$json$Json$Decode$field, 'token', $elm$json$Json$Decode$string));
		case 'registerFailure':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsRegisterFailure,
				A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string));
		case 'forgotPasswordSuccess':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsForgotPasswordSuccess,
				A2($elm$json$Json$Decode$field, 'message', $elm$json$Json$Decode$string));
		case 'forgotPasswordFailure':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsForgotPasswordFailure,
				A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string));
		case 'resetPasswordSuccess':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsResetPasswordSuccess,
				A2($elm$json$Json$Decode$field, 'message', $elm$json$Json$Decode$string));
		case 'resetPasswordFailure':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsResetPasswordFailure,
				A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string));
		case 'checkAuthSuccess':
			return A3(
				$elm$json$Json$Decode$map2,
				$author$project$Main$JsCheckAuthSuccess,
				A2($elm$json$Json$Decode$field, 'user', $author$project$Main$userDecoder),
				A2($elm$json$Json$Decode$field, 'token', $elm$json$Json$Decode$string));
		case 'checkAuthFailure':
			return $elm$json$Json$Decode$succeed($author$project$Main$JsCheckAuthFailure);
		case 'sessionCreated':
			return A3(
				$elm$json$Json$Decode$map2,
				$author$project$Main$JsSessionCreated,
				A2($elm$json$Json$Decode$field, 'session', $author$project$Main$sessionDecoder),
				A2($elm$json$Json$Decode$field, 'gameState', $author$project$Main$gameStateDecoder));
		case 'sessionJoined':
			return A3(
				$elm$json$Json$Decode$map2,
				$author$project$Main$JsSessionJoined,
				A2($elm$json$Json$Decode$field, 'session', $author$project$Main$sessionDecoder),
				A2($elm$json$Json$Decode$field, 'gameState', $author$project$Main$gameStateDecoder));
		case 'sessionLeft':
			return $elm$json$Json$Decode$succeed($author$project$Main$JsSessionLeft);
		case 'readySet':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsReadySet,
				A2($elm$json$Json$Decode$field, 'gameStarted', $elm$json$Json$Decode$bool));
		case 'sessionError':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsSessionError,
				A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string));
		case 'sessionPolled':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsSessionPolled,
				A2($elm$json$Json$Decode$field, 'gameState', $author$project$Main$gameStateDecoder));
		case 'turnStarted':
			return A7(
				$elm$json$Json$Decode$map6,
				$author$project$Main$JsTurnStarted,
				A2($elm$json$Json$Decode$field, 'tile', $elm$json$Json$Decode$string),
				A2($elm$json$Json$Decode$field, 'tileImage', $elm$json$Json$Decode$string),
				A2($elm$json$Json$Decode$field, 'turnNumber', $elm$json$Json$Decode$int),
				A2(
					$elm$json$Json$Decode$field,
					'positions',
					$elm$json$Json$Decode$list($elm$json$Json$Decode$int)),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2(
							$elm$json$Json$Decode$field,
							'players',
							$elm$json$Json$Decode$list($author$project$Main$playerDecoder)),
							$elm$json$Json$Decode$succeed(_List_Nil)
						])),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2(
							$elm$json$Json$Decode$field,
							'waitingForPlayers',
							$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
							$elm$json$Json$Decode$succeed(_List_Nil)
						])));
		case 'movePlayed':
			return A6(
				$elm$json$Json$Decode$map5,
				$author$project$Main$JsMovePlayed,
				A2($elm$json$Json$Decode$field, 'position', $elm$json$Json$Decode$int),
				A2($elm$json$Json$Decode$field, 'points', $elm$json$Json$Decode$int),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2(
							$elm$json$Json$Decode$field,
							'aiTiles',
							$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
							$elm$json$Json$Decode$succeed(_List_Nil)
						])),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2($elm$json$Json$Decode$field, 'aiScore', $elm$json$Json$Decode$int),
							$elm$json$Json$Decode$succeed(0)
						])),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2($elm$json$Json$Decode$field, 'isGameOver', $elm$json$Json$Decode$bool),
							$elm$json$Json$Decode$succeed(false)
						])));
		case 'gameStateUpdated':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsGameStateUpdated,
				A2($elm$json$Json$Decode$field, 'gameState', $author$project$Main$gameStateDecoder));
		case 'gameFinished':
			return A5(
				$elm$json$Json$Decode$map4,
				$author$project$Main$JsGameFinished,
				A2(
					$elm$json$Json$Decode$field,
					'players',
					$elm$json$Json$Decode$list($author$project$Main$playerDecoder)),
				A2(
					$elm$json$Json$Decode$andThen,
					function (_v1) {
						return A2(
							$elm$json$Json$Decode$map,
							function (pairs) {
								return A2(
									$elm$core$Maybe$withDefault,
									A2($elm$core$List$repeat, 19, ''),
									A2(
										$elm$core$Maybe$map,
										$elm$core$Tuple$second,
										$elm$core$List$head(
											A2(
												$elm$core$List$filter,
												function (_v2) {
													var k = _v2.a;
													return k !== 'mcts_ai';
												},
												pairs))));
							},
							A2(
								$elm$json$Json$Decode$field,
								'plateaus',
								$elm$json$Json$Decode$keyValuePairs(
									$elm$json$Json$Decode$list($elm$json$Json$Decode$string))));
					},
					$elm$json$Json$Decode$oneOf(
						_List_fromArray(
							[
								A2(
								$elm$json$Json$Decode$at,
								_List_fromArray(
									['plateaus', 'player']),
								$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
								$elm$json$Json$Decode$succeed(
								A2($elm$core$List$repeat, 19, ''))
							]))),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2(
							$elm$json$Json$Decode$at,
							_List_fromArray(
								['plateaus', 'mcts_ai']),
							$elm$json$Json$Decode$list($elm$json$Json$Decode$string)),
							$elm$json$Json$Decode$succeed(
							A2($elm$core$List$repeat, 19, ''))
						])),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2(
							$elm$json$Json$Decode$field,
							'plateaus',
							$elm$json$Json$Decode$keyValuePairs(
								$elm$json$Json$Decode$list($elm$json$Json$Decode$string))),
							$elm$json$Json$Decode$succeed(_List_Nil)
						])));
		case 'gameError':
			return A2(
				$elm$json$Json$Decode$map,
				$author$project$Main$JsGameError,
				A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string));
		case 'aiMoveResult':
			return A3(
				$elm$json$Json$Decode$map2,
				$author$project$Main$JsAiMoveResult,
				A2($elm$json$Json$Decode$field, 'position', $elm$json$Json$Decode$int),
				$elm$json$Json$Decode$oneOf(
					_List_fromArray(
						[
							A2($elm$json$Json$Decode$field, 'error', $elm$json$Json$Decode$string),
							$elm$json$Json$Decode$succeed('')
						])));
		default:
			return $elm$json$Json$Decode$fail('Unknown message type: ' + msgType);
	}
};
var $author$project$Main$jsMessageDecoder = A2(
	$elm$json$Json$Decode$andThen,
	$author$project$Main$jsMessageDecoderByType,
	A2($elm$json$Json$Decode$field, 'type', $elm$json$Json$Decode$string));
var $elm$browser$Browser$Navigation$load = _Browser_load;
var $elm$core$Platform$Cmd$none = $elm$core$Platform$Cmd$batch(_List_Nil);
var $elm$browser$Browser$Navigation$pushUrl = _Browser_pushUrl;
var $elm$json$Json$Encode$list = F2(
	function (func, entries) {
		return _Json_wrap(
			A3(
				$elm$core$List$foldl,
				_Json_addEntry(func),
				_Json_emptyArray(_Utils_Tuple0),
				entries));
	});
var $elm$core$Process$sleep = _Process_sleep;
var $author$project$Main$resolveCmdIntent = function (intent) {
	switch (intent.$) {
		case 'NoCmd':
			return $elm$core$Platform$Cmd$none;
		case 'SendStartTurn':
			var sessionId = intent.a;
			return $author$project$Main$sendToJs(
				$elm$json$Json$Encode$object(
					_List_fromArray(
						[
							_Utils_Tuple2(
							'type',
							$elm$json$Json$Encode$string('startTurn')),
							_Utils_Tuple2(
							'sessionId',
							$elm$json$Json$Encode$string(sessionId))
						])));
		case 'SchedulePollTurn':
			var delay = intent.a;
			return A2(
				$elm$core$Task$perform,
				function (_v1) {
					return $author$project$Main$PollTurn;
				},
				$elm$core$Process$sleep(delay));
		case 'SendGetAiMove':
			var tileCode = intent.a;
			var boardState = intent.b;
			var availPos = intent.c;
			var turnNum = intent.d;
			return $author$project$Main$sendToJs(
				$elm$json$Json$Encode$object(
					_List_fromArray(
						[
							_Utils_Tuple2(
							'type',
							$elm$json$Json$Encode$string('getAiMove')),
							_Utils_Tuple2(
							'tileCode',
							$elm$json$Json$Encode$string(tileCode)),
							_Utils_Tuple2(
							'boardState',
							A2($elm$json$Json$Encode$list, $elm$json$Json$Encode$string, boardState)),
							_Utils_Tuple2(
							'availablePositions',
							A2($elm$json$Json$Encode$list, $elm$json$Json$Encode$int, availPos)),
							_Utils_Tuple2(
							'turnNumber',
							$elm$json$Json$Encode$int(turnNum))
						])));
		default:
			var cmds = intent.a;
			return $elm$core$Platform$Cmd$batch(
				A2($elm$core$List$map, $author$project$Main$resolveCmdIntent, cmds));
	}
};
var $author$project$Main$toGameModel = function (model) {
	return {
		aiPlateauTiles: model.aiPlateauTiles,
		availablePositions: model.availablePositions,
		currentTile: model.currentTile,
		currentTileImage: model.currentTileImage,
		currentTurnNumber: model.currentTurnNumber,
		gameStateIsFinished: A2(
			$elm$core$Maybe$withDefault,
			false,
			A2(
				$elm$core$Maybe$map,
				function (gs) {
					return _Utils_eq(gs.state, $author$project$Main$Finished);
				},
				model.gameState)),
		hasGameState: !_Utils_eq(model.gameState, $elm$core$Maybe$Nothing),
		hasSession: !_Utils_eq(model.session, $elm$core$Maybe$Nothing),
		isRealGameMode: model.isRealGameMode,
		isSoloMode: model.isSoloMode,
		loading: model.loading,
		myTurn: model.myTurn,
		pendingAiPosition: model.pendingAiPosition,
		plateauTiles: model.plateauTiles,
		playerId: A2(
			$elm$core$Maybe$map,
			function ($) {
				return $.playerId;
			},
			model.session),
		sessionId: A2(
			$elm$core$Maybe$map,
			function ($) {
				return $.sessionId;
			},
			model.session),
		showTilePicker: model.showTilePicker,
		statusMessage: model.statusMessage,
		usedTiles: model.usedTiles,
		waitingForPlayers: model.waitingForPlayers
	};
};
var $elm$url$Url$addPort = F2(
	function (maybePort, starter) {
		if (maybePort.$ === 'Nothing') {
			return starter;
		} else {
			var port_ = maybePort.a;
			return starter + (':' + $elm$core$String$fromInt(port_));
		}
	});
var $elm$url$Url$addPrefixed = F3(
	function (prefix, maybeSegment, starter) {
		if (maybeSegment.$ === 'Nothing') {
			return starter;
		} else {
			var segment = maybeSegment.a;
			return _Utils_ap(
				starter,
				_Utils_ap(prefix, segment));
		}
	});
var $elm$url$Url$toString = function (url) {
	var http = function () {
		var _v0 = url.protocol;
		if (_v0.$ === 'Http') {
			return 'http://';
		} else {
			return 'https://';
		}
	}();
	return A3(
		$elm$url$Url$addPrefixed,
		'#',
		url.fragment,
		A3(
			$elm$url$Url$addPrefixed,
			'?',
			url.query,
			_Utils_ap(
				A2(
					$elm$url$Url$addPort,
					url.port_,
					_Utils_ap(http, url.host)),
				url.path)));
};
var $author$project$Main$handleJsMessage = F2(
	function (value, model) {
		var _v19 = A2($elm$json$Json$Decode$decodeValue, $author$project$Main$jsMessageDecoder, value);
		if (_v19.$ === 'Ok') {
			var jsMsg = _v19.a;
			switch (jsMsg.$) {
				case 'JsLoginSuccess':
					var user = jsMsg.a;
					var token = jsMsg.b;
					return A2(
						$author$project$Main$update,
						A2($author$project$Main$LoginSuccess, user, token),
						model);
				case 'JsLoginFailure':
					var error = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$LoginFailure(error),
						model);
				case 'JsRegisterSuccess':
					var user = jsMsg.a;
					var token = jsMsg.b;
					return A2(
						$author$project$Main$update,
						A2($author$project$Main$RegisterSuccess, user, token),
						model);
				case 'JsRegisterFailure':
					var error = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$RegisterFailure(error),
						model);
				case 'JsForgotPasswordSuccess':
					var message = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$ForgotPasswordSuccess(message),
						model);
				case 'JsForgotPasswordFailure':
					var error = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$ForgotPasswordFailure(error),
						model);
				case 'JsResetPasswordSuccess':
					var message = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$ResetPasswordSuccess(message),
						model);
				case 'JsResetPasswordFailure':
					var error = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$ResetPasswordFailure(error),
						model);
				case 'JsCheckAuthSuccess':
					var user = jsMsg.a;
					var token = jsMsg.b;
					return A2(
						$author$project$Main$update,
						A2($author$project$Main$CheckAuthSuccess, user, token),
						model);
				case 'JsCheckAuthFailure':
					return A2($author$project$Main$update, $author$project$Main$CheckAuthFailure, model);
				case 'JsSessionCreated':
					var session = jsMsg.a;
					var gameState = jsMsg.b;
					return A2(
						$author$project$Main$update,
						A2($author$project$Main$SessionCreated, session, gameState),
						model);
				case 'JsSessionJoined':
					var session = jsMsg.a;
					var gameState = jsMsg.b;
					return A2(
						$author$project$Main$update,
						A2($author$project$Main$SessionJoined, session, gameState),
						model);
				case 'JsSessionLeft':
					return A2($author$project$Main$update, $author$project$Main$SessionLeft, model);
				case 'JsReadySet':
					var gameStarted = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$ReadySet(gameStarted),
						model);
				case 'JsSessionError':
					var error = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$SessionError(error),
						model);
				case 'JsSessionPolled':
					var gameState = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$SessionPolled(gameState),
						model);
				case 'JsTurnStarted':
					var tile = jsMsg.a;
					var tileImage = jsMsg.b;
					var turnNumber = jsMsg.c;
					var positions = jsMsg.d;
					var players = jsMsg.e;
					var waiting = jsMsg.f;
					return A2(
						$author$project$Main$update,
						A6($author$project$Main$TurnStarted, tile, tileImage, turnNumber, positions, players, waiting),
						model);
				case 'JsMovePlayed':
					var position = jsMsg.a;
					var points = jsMsg.b;
					var aiTiles = jsMsg.c;
					var aiScore = jsMsg.d;
					var isGameOver = jsMsg.e;
					return A2(
						$author$project$Main$update,
						A5($author$project$Main$MovePlayed, position, points, aiTiles, aiScore, isGameOver),
						model);
				case 'JsGameStateUpdated':
					var gameState = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$GameStateUpdated(gameState),
						model);
				case 'JsGameFinished':
					var players = jsMsg.a;
					var playerTiles = jsMsg.b;
					var aiTiles = jsMsg.c;
					var allPlateaus = jsMsg.d;
					return A2(
						$author$project$Main$update,
						A4($author$project$Main$GameFinished, players, playerTiles, aiTiles, allPlateaus),
						model);
				case 'JsGameError':
					var error = jsMsg.a;
					return A2(
						$author$project$Main$update,
						$author$project$Main$GameError(error),
						model);
				default:
					var position = jsMsg.a;
					var error = jsMsg.b;
					return A2(
						$author$project$Main$update,
						A2($author$project$Main$AiMoveResult, position, error),
						model);
			}
		} else {
			return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
		}
	});
var $author$project$Main$update = F2(
	function (msg, model) {
		switch (msg.$) {
			case 'UrlRequested':
				var urlRequest = msg.a;
				if (urlRequest.$ === 'Internal') {
					var url = urlRequest.a;
					return _Utils_Tuple2(
						model,
						A2(
							$elm$browser$Browser$Navigation$pushUrl,
							model.key,
							$elm$url$Url$toString(url)));
				} else {
					var href = urlRequest.a;
					return _Utils_Tuple2(
						model,
						$elm$browser$Browser$Navigation$load(href));
				}
			case 'UrlChanged':
				var url = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{url: url}),
					$elm$core$Platform$Cmd$none);
			case 'SetEmailInput':
				var email = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{emailInput: email}),
					$elm$core$Platform$Cmd$none);
			case 'SetUsernameInput':
				var username = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{usernameInput: username}),
					$elm$core$Platform$Cmd$none);
			case 'SetPasswordInput':
				var password = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{passwordInput: password}),
					$elm$core$Platform$Cmd$none);
			case 'SetConfirmPasswordInput':
				var password = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{confirmPasswordInput: password}),
					$elm$core$Platform$Cmd$none);
			case 'SwitchAuthView':
				var newAuthView = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authView: newAuthView, confirmPasswordInput: '', emailInput: '', passwordInput: '', usernameInput: ''}),
					$elm$core$Platform$Cmd$none);
			case 'SkipAuth':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{currentView: $author$project$Main$ModeSelectionView, isAuthenticated: false}),
					$elm$core$Platform$Cmd$none);
			case 'GoToLogin':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authView: $author$project$Main$Login, currentView: $author$project$Main$LoginView, emailInput: '', passwordInput: ''}),
					$elm$core$Platform$Cmd$none);
			case 'SubmitLogin':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authLoading: true}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('login')),
									_Utils_Tuple2(
									'email',
									$elm$json$Json$Encode$string(model.emailInput)),
									_Utils_Tuple2(
									'password',
									$elm$json$Json$Encode$string(model.passwordInput))
								]))));
			case 'SubmitRegister':
				return (!_Utils_eq(model.passwordInput, model.confirmPasswordInput)) ? _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: 'Les mots de passe ne correspondent pas'}),
					$elm$core$Platform$Cmd$none) : _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authLoading: true}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('register')),
									_Utils_Tuple2(
									'email',
									$elm$json$Json$Encode$string(model.emailInput)),
									_Utils_Tuple2(
									'username',
									$elm$json$Json$Encode$string(model.usernameInput)),
									_Utils_Tuple2(
									'password',
									$elm$json$Json$Encode$string(model.passwordInput))
								]))));
			case 'SubmitForgotPassword':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authLoading: true, resetMessage: ''}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('forgotPassword')),
									_Utils_Tuple2(
									'email',
									$elm$json$Json$Encode$string(model.emailInput))
								]))));
			case 'SubmitResetPassword':
				return (!_Utils_eq(model.passwordInput, model.confirmPasswordInput)) ? _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: 'Les mots de passe ne correspondent pas'}),
					$elm$core$Platform$Cmd$none) : _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authLoading: true}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('resetPassword')),
									_Utils_Tuple2(
									'token',
									$elm$json$Json$Encode$string(model.resetToken)),
									_Utils_Tuple2(
									'newPassword',
									$elm$json$Json$Encode$string(model.passwordInput))
								]))));
			case 'Logout':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authView: $author$project$Main$Welcome, currentView: $author$project$Main$LoginView, isAuthenticated: false, token: $elm$core$Maybe$Nothing, user: $elm$core$Maybe$Nothing}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('logout'))
								]))));
			case 'CheckAuth':
				return _Utils_Tuple2(
					model,
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('checkAuth'))
								]))));
			case 'LoginSuccess':
				var user = msg.a;
				var token = msg.b;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							authError: '',
							authLoading: false,
							currentView: $author$project$Main$ModeSelectionView,
							isAuthenticated: true,
							playerName: user.username,
							token: $elm$core$Maybe$Just(token),
							user: $elm$core$Maybe$Just(user)
						}),
					$elm$core$Platform$Cmd$none);
			case 'LoginFailure':
				var error = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: error, authLoading: false}),
					$elm$core$Platform$Cmd$none);
			case 'RegisterSuccess':
				var user = msg.a;
				var token = msg.b;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							authError: '',
							authLoading: false,
							currentView: $author$project$Main$ModeSelectionView,
							isAuthenticated: true,
							playerName: user.username,
							token: $elm$core$Maybe$Just(token),
							user: $elm$core$Maybe$Just(user)
						}),
					$elm$core$Platform$Cmd$none);
			case 'RegisterFailure':
				var error = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: error, authLoading: false}),
					$elm$core$Platform$Cmd$none);
			case 'ForgotPasswordSuccess':
				var message = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authLoading: false, resetMessage: message}),
					$elm$core$Platform$Cmd$none);
			case 'ForgotPasswordFailure':
				var error = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: error, authLoading: false}),
					$elm$core$Platform$Cmd$none);
			case 'ResetPasswordSuccess':
				var message = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: '', authLoading: false, authView: $author$project$Main$Login, confirmPasswordInput: '', passwordInput: '', resetMessage: message, resetToken: ''}),
					$elm$core$Platform$Cmd$none);
			case 'ResetPasswordFailure':
				var error = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{authError: error, authLoading: false}),
					$elm$core$Platform$Cmd$none);
			case 'CheckAuthSuccess':
				var user = msg.a;
				var token = msg.b;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							currentView: $author$project$Main$ModeSelectionView,
							isAuthenticated: true,
							playerName: user.username,
							token: $elm$core$Maybe$Just(token),
							user: $elm$core$Maybe$Just(user)
						}),
					$elm$core$Platform$Cmd$none);
			case 'CheckAuthFailure':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{isAuthenticated: false, token: $elm$core$Maybe$Nothing, user: $elm$core$Maybe$Nothing}),
					$elm$core$Platform$Cmd$none);
			case 'SelectGameMode':
				var mode = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							selectedGameMode: $elm$core$Maybe$Just(mode)
						}),
					$elm$core$Platform$Cmd$none);
			case 'StartGame':
				var _v2 = model.selectedGameMode;
				if (_v2.$ === 'Just') {
					var mode = _v2.a;
					return (mode.id === 'real-game') ? _Utils_Tuple2(
						_Utils_update(
							model,
							{
								availablePositions: A2($elm$core$List$range, 0, 18),
								currentTile: $elm$core$Maybe$Nothing,
								currentTileImage: $elm$core$Maybe$Nothing,
								currentTurnNumber: 0,
								currentView: $author$project$Main$GameView,
								isRealGameMode: true,
								myTurn: true,
								plateauTiles: A2($elm$core$List$repeat, 19, ''),
								realGameScore: 0,
								showTilePicker: true,
								usedTiles: _List_Nil
							}),
						$elm$core$Platform$Cmd$none) : _Utils_Tuple2(
						_Utils_update(
							model,
							{
								aiPlateauTiles: A2($elm$core$List$repeat, 19, ''),
								aiScore: 0,
								allPlayerPlateaus: _List_Nil,
								availablePositions: A2($elm$core$List$range, 0, 18),
								currentTile: $elm$core$Maybe$Nothing,
								currentTileImage: $elm$core$Maybe$Nothing,
								currentTurnNumber: 0,
								currentView: $author$project$Main$GameView,
								isRealGameMode: false,
								plateauTiles: A2($elm$core$List$repeat, 19, ''),
								showAiBoard: false
							}),
						$elm$core$Platform$Cmd$none);
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 'BackToModeSelection':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							aiPlateauTiles: A2($elm$core$List$repeat, 19, ''),
							aiScore: 0,
							allPlayerPlateaus: _List_Nil,
							availablePositions: A2($elm$core$List$range, 0, 18),
							currentTile: $elm$core$Maybe$Nothing,
							currentTileImage: $elm$core$Maybe$Nothing,
							currentTurnNumber: 0,
							currentView: $author$project$Main$ModeSelectionView,
							error: '',
							gameState: $elm$core$Maybe$Nothing,
							isRealGameMode: false,
							loading: false,
							myTurn: false,
							pendingAiPosition: $elm$core$Maybe$Nothing,
							plateauTiles: A2($elm$core$List$repeat, 19, ''),
							realGameScore: 0,
							selectedGameMode: $elm$core$Maybe$Nothing,
							session: $elm$core$Maybe$Nothing,
							showAiBoard: false,
							showTilePicker: false,
							statusMessage: '',
							usedTiles: _List_Nil,
							waitingForPlayers: _List_Nil
						}),
					$elm$core$Platform$Cmd$none);
			case 'ToggleAiBoard':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{showAiBoard: !model.showAiBoard}),
					$elm$core$Platform$Cmd$none);
			case 'RestartSoloGame':
				var gameMode = A2(
					$elm$core$Maybe$withDefault,
					'single-player',
					A2(
						$elm$core$Maybe$map,
						function ($) {
							return $.id;
						},
						model.selectedGameMode));
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							aiPlateauTiles: A2($elm$core$List$repeat, 19, ''),
							aiScore: 0,
							allPlayerPlateaus: _List_Nil,
							availablePositions: A2($elm$core$List$range, 0, 18),
							currentTile: $elm$core$Maybe$Nothing,
							currentTileImage: $elm$core$Maybe$Nothing,
							currentTurnNumber: 0,
							error: '',
							gameState: $elm$core$Maybe$Nothing,
							loading: true,
							plateauTiles: A2($elm$core$List$repeat, 19, ''),
							session: $elm$core$Maybe$Nothing,
							showAiBoard: false,
							statusMessage: ''
						}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('createSession')),
									_Utils_Tuple2(
									'playerName',
									$elm$json$Json$Encode$string(model.playerName)),
									_Utils_Tuple2(
									'gameMode',
									$elm$json$Json$Encode$string(gameMode))
								]))));
			case 'SetPlayerName':
				var name = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{playerName: name}),
					$elm$core$Platform$Cmd$none);
			case 'SetSessionCode':
				var code = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{sessionCode: code}),
					$elm$core$Platform$Cmd$none);
			case 'CreateSession':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{error: '', loading: true}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('createSession')),
									_Utils_Tuple2(
									'playerName',
									$elm$json$Json$Encode$string(model.playerName)),
									_Utils_Tuple2(
									'gameMode',
									$elm$json$Json$Encode$string(
										A2(
											$elm$core$Maybe$withDefault,
											'multiplayer',
											A2(
												$elm$core$Maybe$map,
												function ($) {
													return $.id;
												},
												model.selectedGameMode))))
								]))));
			case 'JoinSession':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{error: '', loading: true}),
					$author$project$Main$sendToJs(
						$elm$json$Json$Encode$object(
							_List_fromArray(
								[
									_Utils_Tuple2(
									'type',
									$elm$json$Json$Encode$string('joinSession')),
									_Utils_Tuple2(
									'sessionCode',
									$elm$json$Json$Encode$string(model.sessionCode)),
									_Utils_Tuple2(
									'playerName',
									$elm$json$Json$Encode$string(model.playerName))
								]))));
			case 'LeaveSession':
				var _v3 = model.session;
				if (_v3.$ === 'Just') {
					var session = _v3.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{loading: true}),
						$author$project$Main$sendToJs(
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'type',
										$elm$json$Json$Encode$string('leaveSession')),
										_Utils_Tuple2(
										'sessionId',
										$elm$json$Json$Encode$string(session.sessionId)),
										_Utils_Tuple2(
										'playerId',
										$elm$json$Json$Encode$string(session.playerId))
									]))));
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 'SetReady':
				var _v4 = model.session;
				if (_v4.$ === 'Just') {
					var session = _v4.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{loading: true}),
						$author$project$Main$sendToJs(
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'type',
										$elm$json$Json$Encode$string('setReady')),
										_Utils_Tuple2(
										'sessionId',
										$elm$json$Json$Encode$string(session.sessionId)),
										_Utils_Tuple2(
										'playerId',
										$elm$json$Json$Encode$string(session.playerId))
									]))));
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 'SessionCreated':
				var session = msg.a;
				var gameState = msg.b;
				var isSoloMode = function () {
					var _v7 = model.selectedGameMode;
					if (_v7.$ === 'Just') {
						var mode = _v7.a;
						return A2($elm$core$String$startsWith, 'single-player', mode.id);
					} else {
						return false;
					}
				}();
				var cmd = isSoloMode ? $elm$core$Platform$Cmd$batch(
					_List_fromArray(
						[
							$author$project$Main$sendToJs(
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'type',
										$elm$json$Json$Encode$string('setReady')),
										_Utils_Tuple2(
										'sessionId',
										$elm$json$Json$Encode$string(session.sessionId)),
										_Utils_Tuple2(
										'playerId',
										$elm$json$Json$Encode$string(session.playerId))
									]))),
							A2(
							$elm$core$Task$perform,
							function (_v5) {
								return $author$project$Main$PollSession;
							},
							$elm$core$Process$sleep(5000))
						])) : A2(
					$elm$core$Task$perform,
					function (_v6) {
						return $author$project$Main$PollSession;
					},
					$elm$core$Process$sleep(2000));
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							gameState: $elm$core$Maybe$Just(gameState),
							isSoloMode: isSoloMode,
							loading: isSoloMode,
							session: $elm$core$Maybe$Just(session),
							statusMessage: 'Session créée: ' + session.sessionCode
						}),
					cmd);
			case 'SessionJoined':
				var session = msg.a;
				var gameState = msg.b;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							gameState: $elm$core$Maybe$Just(gameState),
							loading: false,
							session: $elm$core$Maybe$Just(session),
							statusMessage: 'Rejoint la session: ' + session.sessionCode
						}),
					A2(
						$elm$core$Task$perform,
						function (_v8) {
							return $author$project$Main$PollSession;
						},
						$elm$core$Process$sleep(2000)));
			case 'SessionLeft':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							aiPlateauTiles: A2($elm$core$List$repeat, 19, ''),
							aiScore: 0,
							allPlayerPlateaus: _List_Nil,
							availablePositions: A2($elm$core$List$range, 0, 18),
							currentTile: $elm$core$Maybe$Nothing,
							currentTileImage: $elm$core$Maybe$Nothing,
							currentTurnNumber: 0,
							currentView: $author$project$Main$ModeSelectionView,
							gameState: $elm$core$Maybe$Nothing,
							loading: false,
							myTurn: false,
							plateauTiles: A2($elm$core$List$repeat, 19, ''),
							session: $elm$core$Maybe$Nothing,
							showAiBoard: false,
							waitingForPlayers: _List_Nil
						}),
					$elm$core$Platform$Cmd$none);
			case 'ReadySet':
				var gameStarted = msg.a;
				var newStatusMessage = gameStarted ? 'La partie commence!' : 'Prêt! En attente des autres joueurs...';
				var cmd = gameStarted ? $elm$core$Platform$Cmd$batch(
					_List_fromArray(
						[
							function () {
							var _v9 = model.session;
							if (_v9.$ === 'Just') {
								var session = _v9.a;
								return $author$project$Main$sendToJs(
									$elm$json$Json$Encode$object(
										_List_fromArray(
											[
												_Utils_Tuple2(
												'type',
												$elm$json$Json$Encode$string('startTurn')),
												_Utils_Tuple2(
												'sessionId',
												$elm$json$Json$Encode$string(session.sessionId))
											])));
							} else {
								return $elm$core$Platform$Cmd$none;
							}
						}(),
							A2(
							$elm$core$Task$perform,
							function (_v10) {
								return $author$project$Main$PollTurn;
							},
							$elm$core$Process$sleep(3000))
						])) : A2(
					$elm$core$Task$perform,
					function (_v11) {
						return $author$project$Main$PollSession;
					},
					$elm$core$Process$sleep(2000));
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{loading: gameStarted, statusMessage: newStatusMessage}),
					cmd);
			case 'SessionError':
				var error = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{error: error, loading: false}),
					$elm$core$Platform$Cmd$none);
			case 'PollSession':
				var _v12 = model.session;
				if (_v12.$ === 'Just') {
					var session = _v12.a;
					return _Utils_Tuple2(
						model,
						$author$project$Main$sendToJs(
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'type',
										$elm$json$Json$Encode$string('pollSession')),
										_Utils_Tuple2(
										'sessionId',
										$elm$json$Json$Encode$string(session.sessionId))
									]))));
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 'SessionPolled':
				var gameState = msg.a;
				var gameStarted = _Utils_eq(gameState.state, $author$project$Main$InProgress);
				var autoStartCmd = gameStarted ? $elm$core$Platform$Cmd$batch(
					_List_fromArray(
						[
							function () {
							var _v13 = model.session;
							if (_v13.$ === 'Just') {
								var session = _v13.a;
								return $author$project$Main$sendToJs(
									$elm$json$Json$Encode$object(
										_List_fromArray(
											[
												_Utils_Tuple2(
												'type',
												$elm$json$Json$Encode$string('startTurn')),
												_Utils_Tuple2(
												'sessionId',
												$elm$json$Json$Encode$string(session.sessionId))
											])));
							} else {
								return $elm$core$Platform$Cmd$none;
							}
						}(),
							A2(
							$elm$core$Task$perform,
							function (_v14) {
								return $author$project$Main$PollTurn;
							},
							$elm$core$Process$sleep(3000))
						])) : A2(
					$elm$core$Task$perform,
					function (_v15) {
						return $author$project$Main$PollSession;
					},
					$elm$core$Process$sleep(2000));
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							gameState: $elm$core$Maybe$Just(gameState),
							loading: gameStarted
						}),
					autoStartCmd);
			case 'StartTurn':
				var _v16 = model.session;
				if (_v16.$ === 'Just') {
					var session = _v16.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{loading: true}),
						$author$project$Main$sendToJs(
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'type',
										$elm$json$Json$Encode$string('startTurn')),
										_Utils_Tuple2(
										'sessionId',
										$elm$json$Json$Encode$string(session.sessionId))
									]))));
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 'PlayMove':
				var position = msg.a;
				var _v17 = model.session;
				if (_v17.$ === 'Just') {
					var session = _v17.a;
					return _Utils_Tuple2(
						_Utils_update(
							model,
							{loading: true}),
						$author$project$Main$sendToJs(
							$elm$json$Json$Encode$object(
								_List_fromArray(
									[
										_Utils_Tuple2(
										'type',
										$elm$json$Json$Encode$string('playMove')),
										_Utils_Tuple2(
										'sessionId',
										$elm$json$Json$Encode$string(session.sessionId)),
										_Utils_Tuple2(
										'playerId',
										$elm$json$Json$Encode$string(session.playerId)),
										_Utils_Tuple2(
										'position',
										$elm$json$Json$Encode$int(position))
									]))));
				} else {
					return _Utils_Tuple2(model, $elm$core$Platform$Cmd$none);
				}
			case 'OpenTilePicker':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{showTilePicker: true}),
					$elm$core$Platform$Cmd$none);
			case 'SelectRealTile':
				var tileCode = msg.a;
				var result = A2(
					$author$project$GameLogic$handleSelectRealTilePure,
					$author$project$Main$toGameModel(model),
					tileCode);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{currentTile: result.currentTile, currentTileImage: result.currentTileImage, showTilePicker: result.showTilePicker, usedTiles: result.usedTiles}),
					$author$project$Main$resolveCmdIntent(result.cmdIntent));
			case 'PlaceRealTile':
				var position = msg.a;
				var result = A2(
					$author$project$GameLogic$handlePlaceRealTilePure,
					$author$project$Main$toGameModel(model),
					position);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aiPlateauTiles: result.aiPlateauTiles, availablePositions: result.availablePositions, currentTile: result.currentTile, currentTileImage: result.currentTileImage, currentTurnNumber: result.currentTurnNumber, pendingAiPosition: result.pendingAiPosition, plateauTiles: result.plateauTiles, showTilePicker: result.showTilePicker, statusMessage: result.statusMessage}),
					$elm$core$Platform$Cmd$none);
			case 'ResetRealGame':
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							aiPlateauTiles: A2($elm$core$List$repeat, 19, ''),
							availablePositions: A2($elm$core$List$range, 0, 18),
							currentTile: $elm$core$Maybe$Nothing,
							currentTileImage: $elm$core$Maybe$Nothing,
							currentTurnNumber: 0,
							pendingAiPosition: $elm$core$Maybe$Nothing,
							plateauTiles: A2($elm$core$List$repeat, 19, ''),
							realGameScore: 0,
							showTilePicker: true,
							statusMessage: '',
							usedTiles: _List_Nil
						}),
					$elm$core$Platform$Cmd$none);
			case 'AiMoveResult':
				var position = msg.a;
				var errorMsg = msg.b;
				var result = A2($author$project$GameLogic$handleAiMoveResultPure, position, errorMsg);
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{pendingAiPosition: result.pendingAiPosition, statusMessage: result.statusMessage}),
					$elm$core$Platform$Cmd$none);
			case 'TurnStarted':
				var tile = msg.a;
				var tileImage = msg.b;
				var turnNumber = msg.c;
				var positions = msg.d;
				var players = msg.e;
				var waiting = msg.f;
				var updatedGameState = A2(
					$elm$core$Maybe$map,
					function (gs) {
						return _Utils_update(
							gs,
							{
								players: $elm$core$List$isEmpty(players) ? gs.players : players,
								state: $author$project$Main$InProgress
							});
					},
					model.gameState);
				var result = A2(
					$author$project$GameLogic$handleTurnStartedPure,
					$author$project$Main$toGameModel(model),
					{positions: positions, tile: tile, tileImage: tileImage, turnNumber: turnNumber, waiting: waiting});
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{availablePositions: result.availablePositions, currentTile: result.currentTile, currentTileImage: result.currentTileImage, currentTurnNumber: result.currentTurnNumber, gameState: updatedGameState, loading: result.loading, myTurn: result.myTurn, waitingForPlayers: result.waitingForPlayers}),
					$author$project$Main$resolveCmdIntent(result.cmdIntent));
			case 'MovePlayed':
				var position = msg.a;
				var points = msg.b;
				var aiTiles = msg.c;
				var aiScore = msg.d;
				var isGameOver = msg.e;
				var result = A2(
					$author$project$GameLogic$handleMovePlayedPure,
					$author$project$Main$toGameModel(model),
					{aiScore: aiScore, aiTiles: aiTiles, isGameOver: isGameOver, points: points, position: position});
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aiPlateauTiles: result.aiPlateauTiles, aiScore: aiScore, availablePositions: result.availablePositions, currentTile: result.currentTile, currentTileImage: result.currentTileImage, loading: result.loading, myTurn: result.myTurn, plateauTiles: result.plateauTiles, statusMessage: result.statusMessage}),
					$author$project$Main$resolveCmdIntent(result.cmdIntent));
			case 'GameStateUpdated':
				var gameState = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{
							gameState: $elm$core$Maybe$Just(gameState)
						}),
					$elm$core$Platform$Cmd$none);
			case 'GameFinished':
				var players = msg.a;
				var playerTiles = msg.b;
				var aiTiles = msg.c;
				var allPlateaus = msg.d;
				var mergePlayerScores = F2(
					function (existingPlayers, newPlayers) {
						return A2(
							$elm$core$List$map,
							function (newP) {
								var existingName = A2(
									$elm$core$Maybe$withDefault,
									newP.name,
									A2(
										$elm$core$Maybe$map,
										function ($) {
											return $.name;
										},
										$elm$core$List$head(
											A2(
												$elm$core$List$filter,
												function (p) {
													return _Utils_eq(p.id, newP.id);
												},
												existingPlayers))));
								return _Utils_update(
									newP,
									{name: existingName});
							},
							newPlayers);
					});
				var mergedPlayers = function () {
					var _v18 = model.gameState;
					if (_v18.$ === 'Just') {
						var gs = _v18.a;
						return A2(mergePlayerScores, gs.players, players);
					} else {
						return players;
					}
				}();
				var newGameState = A2(
					$elm$core$Maybe$map,
					function (gs) {
						return _Utils_update(
							gs,
							{players: mergedPlayers, state: $author$project$Main$Finished});
					},
					model.gameState);
				var simplePlayers = A2(
					$elm$core$List$map,
					function (p) {
						return {id: p.id, name: p.name, score: p.score};
					},
					mergedPlayers);
				var result = $author$project$GameLogic$handleGameFinishedPure(
					{aiTiles: aiTiles, allPlateaus: allPlateaus, playerTiles: playerTiles, players: simplePlayers});
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{aiPlateauTiles: result.aiPlateauTiles, allPlayerPlateaus: result.allPlayerPlateaus, error: '', gameState: newGameState, myTurn: result.myTurn, plateauTiles: result.plateauTiles, statusMessage: result.statusMessage, waitingForPlayers: result.waitingForPlayers}),
					$elm$core$Platform$Cmd$none);
			case 'GameError':
				var error = msg.a;
				return _Utils_Tuple2(
					_Utils_update(
						model,
						{error: error, loading: false}),
					$elm$core$Platform$Cmd$none);
			case 'PollTurn':
				return _Utils_Tuple2(
					model,
					$author$project$Main$resolveCmdIntent(
						$author$project$GameLogic$handlePollTurnPure(
							$author$project$Main$toGameModel(model))));
			default:
				var value = msg.a;
				return A2($author$project$Main$handleJsMessage, value, model);
		}
	});
var $elm$html$Html$Attributes$stringProperty = F2(
	function (key, string) {
		return A2(
			_VirtualDom_property,
			key,
			$elm$json$Json$Encode$string(string));
	});
var $elm$html$Html$Attributes$class = $elm$html$Html$Attributes$stringProperty('className');
var $elm$html$Html$div = _VirtualDom_node('div');
var $author$project$Main$authSubtitle = function (authView) {
	switch (authView.$) {
		case 'Welcome':
			return 'Le jeu de stratégie et de chance';
		case 'Login':
			return 'Connectez-vous pour jouer';
		case 'Register':
			return 'Créez votre compte';
		case 'ForgotPassword':
			return 'Réinitialiser votre mot de passe';
		default:
			return 'Choisissez un nouveau mot de passe';
	}
};
var $elm$html$Html$h1 = _VirtualDom_node('h1');
var $elm$html$Html$p = _VirtualDom_node('p');
var $elm$virtual_dom$VirtualDom$text = _VirtualDom_text;
var $elm$html$Html$text = $elm$virtual_dom$VirtualDom$text;
var $author$project$Main$Register = {$: 'Register'};
var $author$project$Main$SkipAuth = {$: 'SkipAuth'};
var $author$project$Main$SwitchAuthView = function (a) {
	return {$: 'SwitchAuthView', a: a};
};
var $elm$html$Html$button = _VirtualDom_node('button');
var $elm$json$Json$Encode$bool = _Json_wrap;
var $elm$html$Html$Attributes$boolProperty = F2(
	function (key, bool) {
		return A2(
			_VirtualDom_property,
			key,
			$elm$json$Json$Encode$bool(bool));
	});
var $elm$html$Html$Attributes$disabled = $elm$html$Html$Attributes$boolProperty('disabled');
var $elm$virtual_dom$VirtualDom$Normal = function (a) {
	return {$: 'Normal', a: a};
};
var $elm$virtual_dom$VirtualDom$on = _VirtualDom_on;
var $elm$html$Html$Events$on = F2(
	function (event, decoder) {
		return A2(
			$elm$virtual_dom$VirtualDom$on,
			event,
			$elm$virtual_dom$VirtualDom$Normal(decoder));
	});
var $elm$html$Html$Events$onClick = function (msg) {
	return A2(
		$elm$html$Html$Events$on,
		'click',
		$elm$json$Json$Decode$succeed(msg));
};
var $elm$html$Html$Attributes$type_ = $elm$html$Html$Attributes$stringProperty('type');
var $author$project$Main$viewAuthFooter = function (model) {
	return _Utils_eq(model.authView, $author$project$Main$Welcome) ? $elm$html$Html$text('') : A2(
		$elm$html$Html$div,
		_List_Nil,
		_List_fromArray(
			[
				function () {
				var _v0 = model.authView;
				switch (_v0.$) {
					case 'Login':
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('auth-switch')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$p,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Pas encore de compte ? '),
											A2(
											$elm$html$Html$button,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$type_('button'),
													$elm$html$Html$Attributes$class('auth-switch-button'),
													$elm$html$Html$Events$onClick(
													$author$project$Main$SwitchAuthView($author$project$Main$Register)),
													$elm$html$Html$Attributes$disabled(model.authLoading)
												]),
											_List_fromArray(
												[
													$elm$html$Html$text('S\'inscrire')
												]))
										]))
								]));
					case 'Register':
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('auth-switch')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$p,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Déjà un compte ? '),
											A2(
											$elm$html$Html$button,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$type_('button'),
													$elm$html$Html$Attributes$class('auth-switch-button'),
													$elm$html$Html$Events$onClick(
													$author$project$Main$SwitchAuthView($author$project$Main$Login)),
													$elm$html$Html$Attributes$disabled(model.authLoading)
												]),
											_List_fromArray(
												[
													$elm$html$Html$text('Se connecter')
												]))
										]))
								]));
					default:
						return $elm$html$Html$text('');
				}
			}(),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('auth-skip')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('button'),
								$elm$html$Html$Attributes$class('skip-button'),
								$elm$html$Html$Events$onClick($author$project$Main$SkipAuth),
								$elm$html$Html$Attributes$disabled(model.authLoading)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Jouer en mode invité')
							]))
					]))
			]));
};
var $author$project$Main$SetEmailInput = function (a) {
	return {$: 'SetEmailInput', a: a};
};
var $author$project$Main$SubmitForgotPassword = {$: 'SubmitForgotPassword'};
var $elm$html$Html$Attributes$for = $elm$html$Html$Attributes$stringProperty('htmlFor');
var $elm$html$Html$form = _VirtualDom_node('form');
var $elm$html$Html$Attributes$id = $elm$html$Html$Attributes$stringProperty('id');
var $elm$html$Html$input = _VirtualDom_node('input');
var $elm$html$Html$label = _VirtualDom_node('label');
var $elm$html$Html$Events$alwaysStop = function (x) {
	return _Utils_Tuple2(x, true);
};
var $elm$virtual_dom$VirtualDom$MayStopPropagation = function (a) {
	return {$: 'MayStopPropagation', a: a};
};
var $elm$html$Html$Events$stopPropagationOn = F2(
	function (event, decoder) {
		return A2(
			$elm$virtual_dom$VirtualDom$on,
			event,
			$elm$virtual_dom$VirtualDom$MayStopPropagation(decoder));
	});
var $elm$html$Html$Events$targetValue = A2(
	$elm$json$Json$Decode$at,
	_List_fromArray(
		['target', 'value']),
	$elm$json$Json$Decode$string);
var $elm$html$Html$Events$onInput = function (tagger) {
	return A2(
		$elm$html$Html$Events$stopPropagationOn,
		'input',
		A2(
			$elm$json$Json$Decode$map,
			$elm$html$Html$Events$alwaysStop,
			A2($elm$json$Json$Decode$map, tagger, $elm$html$Html$Events$targetValue)));
};
var $elm$virtual_dom$VirtualDom$MayPreventDefault = function (a) {
	return {$: 'MayPreventDefault', a: a};
};
var $elm$html$Html$Events$preventDefaultOn = F2(
	function (event, decoder) {
		return A2(
			$elm$virtual_dom$VirtualDom$on,
			event,
			$elm$virtual_dom$VirtualDom$MayPreventDefault(decoder));
	});
var $author$project$Main$onSubmitPreventDefault = function (msg) {
	return A2(
		$elm$html$Html$Events$preventDefaultOn,
		'submit',
		$elm$json$Json$Decode$succeed(
			_Utils_Tuple2(msg, true)));
};
var $elm$html$Html$Attributes$placeholder = $elm$html$Html$Attributes$stringProperty('placeholder');
var $elm$html$Html$Attributes$required = $elm$html$Html$Attributes$boolProperty('required');
var $elm$html$Html$span = _VirtualDom_node('span');
var $elm$html$Html$Attributes$value = $elm$html$Html$Attributes$stringProperty('value');
var $author$project$Main$viewForgotPasswordForm = function (model) {
	return A2(
		$elm$html$Html$form,
		_List_fromArray(
			[
				$author$project$Main$onSubmitPreventDefault($author$project$Main$SubmitForgotPassword),
				$elm$html$Html$Attributes$class('auth-form')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('email')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Email')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('email'),
								$elm$html$Html$Attributes$id('email'),
								$elm$html$Html$Attributes$value(model.emailInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetEmailInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$disabled(model.authLoading)
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$button,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('button'),
						$elm$html$Html$Attributes$class('auth-submit-button'),
						$elm$html$Html$Attributes$disabled(model.authLoading),
						$elm$html$Html$Events$onClick($author$project$Main$SubmitForgotPassword)
					]),
				_List_fromArray(
					[
						model.authLoading ? A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('loading-spinner')
							]),
						_List_Nil) : $elm$html$Html$text('Envoyer le lien de réinitialisation')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('back-to-login')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('button'),
								$elm$html$Html$Attributes$class('link-button'),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SwitchAuthView($author$project$Main$Login))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('← Retour à la connexion')
							]))
					]))
			]));
};
var $author$project$Main$ForgotPassword = {$: 'ForgotPassword'};
var $author$project$Main$SetConfirmPasswordInput = function (a) {
	return {$: 'SetConfirmPasswordInput', a: a};
};
var $author$project$Main$SetPasswordInput = function (a) {
	return {$: 'SetPasswordInput', a: a};
};
var $author$project$Main$SetUsernameInput = function (a) {
	return {$: 'SetUsernameInput', a: a};
};
var $author$project$Main$SubmitLogin = {$: 'SubmitLogin'};
var $author$project$Main$SubmitRegister = {$: 'SubmitRegister'};
var $elm$virtual_dom$VirtualDom$attribute = F2(
	function (key, value) {
		return A2(
			_VirtualDom_attribute,
			_VirtualDom_noOnOrFormAction(key),
			_VirtualDom_noJavaScriptOrHtmlUri(value));
	});
var $elm$html$Html$Attributes$attribute = $elm$virtual_dom$VirtualDom$attribute;
var $elm$html$Html$Attributes$maxlength = function (n) {
	return A2(
		_VirtualDom_attribute,
		'maxlength',
		$elm$core$String$fromInt(n));
};
var $elm$html$Html$Attributes$minlength = function (n) {
	return A2(
		_VirtualDom_attribute,
		'minLength',
		$elm$core$String$fromInt(n));
};
var $author$project$Main$viewLoginRegisterForm = function (model) {
	return A2(
		$elm$html$Html$form,
		_List_fromArray(
			[
				$author$project$Main$onSubmitPreventDefault(
				_Utils_eq(model.authView, $author$project$Main$Login) ? $author$project$Main$SubmitLogin : $author$project$Main$SubmitRegister),
				$elm$html$Html$Attributes$class('auth-form')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('email')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Email')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('email'),
								$elm$html$Html$Attributes$id('email'),
								$elm$html$Html$Attributes$value(model.emailInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetEmailInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$disabled(model.authLoading)
							]),
						_List_Nil)
					])),
				_Utils_eq(model.authView, $author$project$Main$Register) ? A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('username')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Nom d\'utilisateur')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('text'),
								$elm$html$Html$Attributes$id('username'),
								$elm$html$Html$Attributes$value(model.usernameInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetUsernameInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$minlength(3),
								$elm$html$Html$Attributes$maxlength(30),
								$elm$html$Html$Attributes$disabled(model.authLoading)
							]),
						_List_Nil)
					])) : $elm$html$Html$text(''),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('password')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Mot de passe')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('password'),
								$elm$html$Html$Attributes$id('password'),
								$elm$html$Html$Attributes$value(model.passwordInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetPasswordInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$minlength(8),
								$elm$html$Html$Attributes$disabled(model.authLoading),
								A2(
								$elm$html$Html$Attributes$attribute,
								'autocomplete',
								_Utils_eq(model.authView, $author$project$Main$Register) ? 'new-password' : 'current-password')
							]),
						_List_Nil)
					])),
				_Utils_eq(model.authView, $author$project$Main$Register) ? A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('confirmPassword')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Confirmer le mot de passe')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('password'),
								$elm$html$Html$Attributes$id('confirmPassword'),
								$elm$html$Html$Attributes$value(model.confirmPasswordInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetConfirmPasswordInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$disabled(model.authLoading),
								A2($elm$html$Html$Attributes$attribute, 'autocomplete', 'new-password')
							]),
						_List_Nil)
					])) : $elm$html$Html$text(''),
				A2(
				$elm$html$Html$button,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('button'),
						$elm$html$Html$Attributes$class('auth-submit-button'),
						$elm$html$Html$Attributes$disabled(model.authLoading),
						$elm$html$Html$Events$onClick(
						_Utils_eq(model.authView, $author$project$Main$Login) ? $author$project$Main$SubmitLogin : $author$project$Main$SubmitRegister)
					]),
				_List_fromArray(
					[
						model.authLoading ? A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('loading-spinner')
							]),
						_List_Nil) : $elm$html$Html$text(
						_Utils_eq(model.authView, $author$project$Main$Login) ? 'Se connecter' : 'Créer mon compte')
					])),
				_Utils_eq(model.authView, $author$project$Main$Login) ? A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('forgot-password-link')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('button'),
								$elm$html$Html$Attributes$class('link-button'),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SwitchAuthView($author$project$Main$ForgotPassword))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Mot de passe oublié ?')
							]))
					])) : $elm$html$Html$text('')
			]));
};
var $author$project$Main$SubmitResetPassword = {$: 'SubmitResetPassword'};
var $author$project$Main$viewResetPasswordForm = function (model) {
	return A2(
		$elm$html$Html$form,
		_List_fromArray(
			[
				$author$project$Main$onSubmitPreventDefault($author$project$Main$SubmitResetPassword),
				$elm$html$Html$Attributes$class('auth-form')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('password')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Nouveau mot de passe')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('password'),
								$elm$html$Html$Attributes$id('password'),
								$elm$html$Html$Attributes$value(model.passwordInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetPasswordInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$minlength(8),
								$elm$html$Html$Attributes$disabled(model.authLoading),
								A2($elm$html$Html$Attributes$attribute, 'autocomplete', 'new-password')
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('confirmPassword')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Confirmer le mot de passe')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('password'),
								$elm$html$Html$Attributes$id('confirmPassword'),
								$elm$html$Html$Attributes$value(model.confirmPasswordInput),
								$elm$html$Html$Events$onInput($author$project$Main$SetConfirmPasswordInput),
								$elm$html$Html$Attributes$placeholder(''),
								$elm$html$Html$Attributes$required(true),
								$elm$html$Html$Attributes$disabled(model.authLoading),
								A2($elm$html$Html$Attributes$attribute, 'autocomplete', 'new-password')
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$button,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$type_('button'),
						$elm$html$Html$Attributes$class('auth-submit-button'),
						$elm$html$Html$Attributes$disabled(model.authLoading),
						$elm$html$Html$Events$onClick($author$project$Main$SubmitResetPassword)
					]),
				_List_fromArray(
					[
						model.authLoading ? A2(
						$elm$html$Html$span,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('loading-spinner')
							]),
						_List_Nil) : $elm$html$Html$text('Réinitialiser le mot de passe')
					]))
			]));
};
var $elm$core$String$fromFloat = _String_fromNumber;
var $author$project$TileSvg$Tile = F3(
	function (v1, v2, v3) {
		return {v1: v1, v2: v2, v3: v3};
	});
var $elm$core$String$cons = _String_cons;
var $elm$core$String$fromChar = function (_char) {
	return A2($elm$core$String$cons, _char, '');
};
var $elm$core$Maybe$map3 = F4(
	function (func, ma, mb, mc) {
		if (ma.$ === 'Nothing') {
			return $elm$core$Maybe$Nothing;
		} else {
			var a = ma.a;
			if (mb.$ === 'Nothing') {
				return $elm$core$Maybe$Nothing;
			} else {
				var b = mb.a;
				if (mc.$ === 'Nothing') {
					return $elm$core$Maybe$Nothing;
				} else {
					var c = mc.a;
					return $elm$core$Maybe$Just(
						A3(func, a, b, c));
				}
			}
		}
	});
var $elm$core$String$foldr = _String_foldr;
var $elm$core$String$toList = function (string) {
	return A3($elm$core$String$foldr, $elm$core$List$cons, _List_Nil, string);
};
var $author$project$TileSvg$parseTileFromPath = function (imagePath) {
	var filename = A3(
		$elm$core$String$replace,
		'.png',
		'',
		A3(
			$elm$core$String$replace,
			'image/',
			'',
			A3($elm$core$String$replace, '../', '', imagePath)));
	var _v0 = $elm$core$String$toList(filename);
	if (((_v0.b && _v0.b.b) && _v0.b.b.b) && (!_v0.b.b.b.b)) {
		var c1 = _v0.a;
		var _v1 = _v0.b;
		var c2 = _v1.a;
		var _v2 = _v1.b;
		var c3 = _v2.a;
		return A4(
			$elm$core$Maybe$map3,
			$author$project$TileSvg$Tile,
			$elm$core$String$toInt(
				$elm$core$String$fromChar(c1)),
			$elm$core$String$toInt(
				$elm$core$String$fromChar(c2)),
			$elm$core$String$toInt(
				$elm$core$String$fromChar(c3)));
	} else {
		return $elm$core$Maybe$Nothing;
	}
};
var $elm$virtual_dom$VirtualDom$style = _VirtualDom_style;
var $elm$html$Html$Attributes$style = $elm$virtual_dom$VirtualDom$style;
var $elm$svg$Svg$trustedNode = _VirtualDom_nodeNS('http://www.w3.org/2000/svg');
var $elm$svg$Svg$clipPath = $elm$svg$Svg$trustedNode('clipPath');
var $elm$svg$Svg$Attributes$clipPath = _VirtualDom_attribute('clip-path');
var $author$project$TileSvg$colorForValue = function (value) {
	switch (value) {
		case 1:
			return '#a0a0a0';
		case 2:
			return '#ffb6c1';
		case 3:
			return '#ff69b4';
		case 4:
			return '#00a0ff';
		case 5:
			return '#00b4a0';
		case 6:
			return '#ff3030';
		case 7:
			return '#a0d800';
		case 8:
			return '#ff8c00';
		case 9:
			return '#f0d000';
		default:
			return '#666666';
	}
};
var $elm$svg$Svg$defs = $elm$svg$Svg$trustedNode('defs');
var $elm$svg$Svg$Attributes$fill = _VirtualDom_attribute('fill');
var $elm$svg$Svg$g = $elm$svg$Svg$trustedNode('g');
var $elm$svg$Svg$Attributes$height = _VirtualDom_attribute('height');
var $elm$svg$Svg$Attributes$id = _VirtualDom_attribute('id');
var $elm$svg$Svg$Attributes$points = _VirtualDom_attribute('points');
var $elm$svg$Svg$polygon = $elm$svg$Svg$trustedNode('polygon');
var $elm$svg$Svg$Attributes$stroke = _VirtualDom_attribute('stroke');
var $elm$svg$Svg$Attributes$strokeWidth = _VirtualDom_attribute('stroke-width');
var $elm$svg$Svg$svg = $elm$svg$Svg$trustedNode('svg');
var $elm$svg$Svg$Attributes$viewBox = _VirtualDom_attribute('viewBox');
var $elm$svg$Svg$rect = $elm$svg$Svg$trustedNode('rect');
var $elm$svg$Svg$Attributes$transform = _VirtualDom_attribute('transform');
var $elm$svg$Svg$Attributes$width = _VirtualDom_attribute('width');
var $elm$svg$Svg$Attributes$x = _VirtualDom_attribute('x');
var $elm$svg$Svg$Attributes$y = _VirtualDom_attribute('y');
var $author$project$TileSvg$viewDiagonalBandLeft = F2(
	function (value, bandWidth) {
		return A2(
			$elm$svg$Svg$g,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$transform('rotate(-60, 50, 43.3)')
				]),
			_List_fromArray(
				[
					A2(
					$elm$svg$Svg$rect,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(50 - (bandWidth / 2))),
							$elm$svg$Svg$Attributes$y('-20'),
							$elm$svg$Svg$Attributes$width(
							$elm$core$String$fromFloat(bandWidth)),
							$elm$svg$Svg$Attributes$height('130'),
							$elm$svg$Svg$Attributes$fill(
							$author$project$TileSvg$colorForValue(value))
						]),
					_List_Nil)
				]));
	});
var $author$project$TileSvg$viewDiagonalBandRight = F2(
	function (value, bandWidth) {
		return A2(
			$elm$svg$Svg$g,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$transform('rotate(60, 50, 43.3)')
				]),
			_List_fromArray(
				[
					A2(
					$elm$svg$Svg$rect,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(50 - (bandWidth / 2))),
							$elm$svg$Svg$Attributes$y('-20'),
							$elm$svg$Svg$Attributes$width(
							$elm$core$String$fromFloat(bandWidth)),
							$elm$svg$Svg$Attributes$height('130'),
							$elm$svg$Svg$Attributes$fill(
							$author$project$TileSvg$colorForValue(value))
						]),
					_List_Nil)
				]));
	});
var $elm$svg$Svg$Attributes$dominantBaseline = _VirtualDom_attribute('dominant-baseline');
var $elm$svg$Svg$Attributes$fontFamily = _VirtualDom_attribute('font-family');
var $elm$svg$Svg$Attributes$fontSize = _VirtualDom_attribute('font-size');
var $elm$svg$Svg$Attributes$fontWeight = _VirtualDom_attribute('font-weight');
var $elm$svg$Svg$text = $elm$virtual_dom$VirtualDom$text;
var $elm$svg$Svg$Attributes$textAnchor = _VirtualDom_attribute('text-anchor');
var $elm$svg$Svg$text_ = $elm$svg$Svg$trustedNode('text');
var $author$project$TileSvg$viewNumber = F4(
	function (value, xPos, yPos, bgColor) {
		return A2(
			$elm$svg$Svg$g,
			_List_Nil,
			_List_fromArray(
				[
					A2(
					$elm$svg$Svg$text_,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(xPos)),
							$elm$svg$Svg$Attributes$y(
							$elm$core$String$fromFloat(yPos)),
							$elm$svg$Svg$Attributes$textAnchor('middle'),
							$elm$svg$Svg$Attributes$dominantBaseline('middle'),
							$elm$svg$Svg$Attributes$fontSize('16'),
							$elm$svg$Svg$Attributes$fontWeight('bold'),
							$elm$svg$Svg$Attributes$fontFamily('Arial, sans-serif'),
							$elm$svg$Svg$Attributes$stroke('#000'),
							$elm$svg$Svg$Attributes$strokeWidth('3'),
							$elm$svg$Svg$Attributes$fill('#000')
						]),
					_List_fromArray(
						[
							$elm$svg$Svg$text(
							$elm$core$String$fromInt(value))
						])),
					A2(
					$elm$svg$Svg$text_,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x(
							$elm$core$String$fromFloat(xPos)),
							$elm$svg$Svg$Attributes$y(
							$elm$core$String$fromFloat(yPos)),
							$elm$svg$Svg$Attributes$textAnchor('middle'),
							$elm$svg$Svg$Attributes$dominantBaseline('middle'),
							$elm$svg$Svg$Attributes$fontSize('16'),
							$elm$svg$Svg$Attributes$fontWeight('bold'),
							$elm$svg$Svg$Attributes$fontFamily('Arial, sans-serif'),
							$elm$svg$Svg$Attributes$fill('#fff')
						]),
					_List_fromArray(
						[
							$elm$svg$Svg$text(
							$elm$core$String$fromInt(value))
						]))
				]));
	});
var $author$project$TileSvg$viewVerticalBand = F2(
	function (value, bandWidth) {
		return A2(
			$elm$svg$Svg$rect,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$x(
					$elm$core$String$fromFloat(50 - (bandWidth / 2))),
					$elm$svg$Svg$Attributes$y('-5'),
					$elm$svg$Svg$Attributes$width(
					$elm$core$String$fromFloat(bandWidth)),
					$elm$svg$Svg$Attributes$height('100'),
					$elm$svg$Svg$Attributes$fill(
					$author$project$TileSvg$colorForValue(value))
				]),
			_List_Nil);
	});
var $author$project$TileSvg$viewTileSvg = function (tile) {
	var width = 100;
	var hexPoints = '25,0 75,0 100,43.3 75,86.6 25,86.6 0,43.3';
	var height = 86.6;
	var bandWidth = 14;
	return A2(
		$elm$svg$Svg$svg,
		_List_fromArray(
			[
				$elm$svg$Svg$Attributes$viewBox('0 0 100 86.6'),
				$elm$svg$Svg$Attributes$width('100%'),
				$elm$svg$Svg$Attributes$height('100%')
			]),
		_List_fromArray(
			[
				A2(
				$elm$svg$Svg$defs,
				_List_Nil,
				_List_fromArray(
					[
						A2(
						$elm$svg$Svg$clipPath,
						_List_fromArray(
							[
								$elm$svg$Svg$Attributes$id('hexClip')
							]),
						_List_fromArray(
							[
								A2(
								$elm$svg$Svg$polygon,
								_List_fromArray(
									[
										$elm$svg$Svg$Attributes$points(hexPoints)
									]),
								_List_Nil)
							]))
					])),
				A2(
				$elm$svg$Svg$polygon,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$points(hexPoints),
						$elm$svg$Svg$Attributes$fill('#1a1a2e'),
						$elm$svg$Svg$Attributes$stroke('#333'),
						$elm$svg$Svg$Attributes$strokeWidth('1')
					]),
				_List_Nil),
				A2(
				$elm$svg$Svg$g,
				_List_fromArray(
					[
						$elm$svg$Svg$Attributes$clipPath('url(#hexClip)')
					]),
				_List_fromArray(
					[
						A2($author$project$TileSvg$viewDiagonalBandRight, tile.v2, bandWidth),
						A2($author$project$TileSvg$viewDiagonalBandLeft, tile.v3, bandWidth),
						A2($author$project$TileSvg$viewVerticalBand, tile.v1, bandWidth)
					])),
				A4(
				$author$project$TileSvg$viewNumber,
				tile.v1,
				50,
				18,
				$author$project$TileSvg$colorForValue(tile.v1)),
				A4(
				$author$project$TileSvg$viewNumber,
				tile.v2,
				22,
				62,
				$author$project$TileSvg$colorForValue(tile.v2)),
				A4(
				$author$project$TileSvg$viewNumber,
				tile.v3,
				78,
				62,
				$author$project$TileSvg$colorForValue(tile.v3))
			]));
};
var $author$project$Main$viewWelcomeBoard = function () {
	var viewScoringOverlay = function (_v8) {
		var phaseClass = _v8.a;
		var delay = _v8.b;
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('scoring-overlay ' + phaseClass),
					A2(
					$elm$html$Html$Attributes$style,
					'animation-delay',
					$elm$core$String$fromFloat(delay) + 's')
				]),
			_List_Nil);
	};
	var tileInterval = 0.4;
	var placementOrder = _List_fromArray(
		[7, 14, 2, 11, 5, 17, 0, 9, 15, 3, 12, 18, 6, 1, 10, 16, 8, 4, 13]);
	var placementStep = function (idx) {
		return A2(
			$elm$core$Maybe$withDefault,
			idx,
			A2(
				$elm$core$Maybe$map,
				$elm$core$Tuple$first,
				$elm$core$List$head(
					A2(
						$elm$core$List$filter,
						function (_v7) {
							var bi = _v7.b;
							return _Utils_eq(bi, idx);
						},
						A2(
							$elm$core$List$indexedMap,
							F2(
								function (s, bi) {
									return _Utils_Tuple2(s, bi);
								}),
							placementOrder)))));
	};
	var hexRadius = 36;
	var hexWidth = 2 * hexRadius;
	var spacingX = 0.75 * hexWidth;
	var hexPositions = _List_fromArray(
		[
			_Utils_Tuple2(0, 1),
			_Utils_Tuple2(0, 2),
			_Utils_Tuple2(0, 3),
			_Utils_Tuple2(1, 0.5),
			_Utils_Tuple2(1, 1.5),
			_Utils_Tuple2(1, 2.5),
			_Utils_Tuple2(1, 3.5),
			_Utils_Tuple2(2, 0),
			_Utils_Tuple2(2, 1),
			_Utils_Tuple2(2, 2),
			_Utils_Tuple2(2, 3),
			_Utils_Tuple2(2, 4),
			_Utils_Tuple2(3, 0.5),
			_Utils_Tuple2(3, 1.5),
			_Utils_Tuple2(3, 2.5),
			_Utils_Tuple2(3, 3.5),
			_Utils_Tuple2(4, 1),
			_Utils_Tuple2(4, 2),
			_Utils_Tuple2(4, 3)
		]);
	var hexHeight = 1.732 * hexRadius;
	var spacingY = hexHeight;
	var gridOriginY = 20;
	var gridOriginX = 16;
	var getScoringOverlays = function (idx) {
		return _Utils_ap(
			A2(
				$elm$core$List$member,
				idx,
				_List_fromArray(
					[0, 1, 2])) ? _List_fromArray(
				[
					_Utils_Tuple2('phase-v1', 8.5)
				]) : _List_Nil,
			_Utils_ap(
				A2(
					$elm$core$List$member,
					idx,
					_List_fromArray(
						[16, 17, 18])) ? _List_fromArray(
					[
						_Utils_Tuple2('phase-v1', 8.5)
					]) : _List_Nil,
				_Utils_ap(
					A2(
						$elm$core$List$member,
						idx,
						_List_fromArray(
							[0, 3, 7])) ? _List_fromArray(
						[
							_Utils_Tuple2('phase-v2', 11.0)
						]) : _List_Nil,
					_Utils_ap(
						A2(
							$elm$core$List$member,
							idx,
							_List_fromArray(
								[11, 15, 18])) ? _List_fromArray(
							[
								_Utils_Tuple2('phase-v2', 11.0)
							]) : _List_Nil,
						A2(
							$elm$core$List$member,
							idx,
							_List_fromArray(
								[2, 6, 11])) ? _List_fromArray(
							[
								_Utils_Tuple2('phase-v3', 13.5)
							]) : _List_Nil))));
	};
	var allTiles = _List_fromArray(
		[
			_Utils_Tuple2(0, '963'),
			_Utils_Tuple2(1, '974'),
			_Utils_Tuple2(2, '928'),
			_Utils_Tuple2(3, '164'),
			_Utils_Tuple2(4, '123'),
			_Utils_Tuple2(5, '524'),
			_Utils_Tuple2(6, '568'),
			_Utils_Tuple2(7, '563'),
			_Utils_Tuple2(8, '173'),
			_Utils_Tuple2(9, '924'),
			_Utils_Tuple2(10, '168'),
			_Utils_Tuple2(11, '178'),
			_Utils_Tuple2(12, '964'),
			_Utils_Tuple2(13, '573'),
			_Utils_Tuple2(14, '124'),
			_Utils_Tuple2(15, '973'),
			_Utils_Tuple2(16, '523'),
			_Utils_Tuple2(17, '528'),
			_Utils_Tuple2(18, '574')
		]);
	var getTileCode = function (idx) {
		return A2(
			$elm$core$Maybe$map,
			$elm$core$Tuple$second,
			$elm$core$List$head(
				A2(
					$elm$core$List$filter,
					function (_v6) {
						var i = _v6.a;
						return _Utils_eq(i, idx);
					},
					allTiles)));
	};
	var orderedTilesForPreview = A2(
		$elm$core$List$filterMap,
		function (boardIdx) {
			return $elm$core$List$head(
				A2(
					$elm$core$List$filter,
					function (_v5) {
						var i = _v5.a;
						return _Utils_eq(i, boardIdx);
					},
					allTiles));
		},
		placementOrder);
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('welcome-board-wrapper')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-tile-preview')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('preview-label')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Tuile a placer')
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('preview-tile-area')
							]),
						A2(
							$elm$core$List$indexedMap,
							F2(
								function (step, _v0) {
									var tileCode = _v0.b;
									var _v1 = $author$project$TileSvg$parseTileFromPath(tileCode);
									if (_v1.$ === 'Just') {
										var tileData = _v1.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class(
													(step === 18) ? 'preview-tile preview-last' : 'preview-tile'),
													A2(
													$elm$html$Html$Attributes$style,
													'animation-delay',
													$elm$core$String$fromFloat(step * tileInterval) + 's')
												]),
											_List_fromArray(
												[
													A2(
													$elm$html$Html$div,
													_List_fromArray(
														[
															$elm$html$Html$Attributes$class('hex-tile-svg')
														]),
													_List_fromArray(
														[
															$author$project$TileSvg$viewTileSvg(tileData)
														]))
												]));
									} else {
										return $elm$html$Html$text('');
									}
								}),
							orderedTilesForPreview)),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('preview-arrow')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('\u2193')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-direction-labels')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('direction-label phase-v1'),
								A2($elm$html$Html$Attributes$style, 'animation-delay', '8.5s')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Colonnes ↕')
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('direction-label phase-v2'),
								A2($elm$html$Html$Attributes$style, 'animation-delay', '11.0s')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Diagonales ↗')
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('direction-label phase-v3'),
								A2($elm$html$Html$Attributes$style, 'animation-delay', '13.5s')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Diagonales ↘')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('hex-board welcome-hex-board'),
						A2($elm$html$Html$Attributes$style, 'position', 'relative'),
						A2($elm$html$Html$Attributes$style, 'width', '320px'),
						A2($elm$html$Html$Attributes$style, 'height', '360px')
					]),
				_Utils_ap(
					A2(
						$elm$core$List$indexedMap,
						F2(
							function (index, _v2) {
								var col = _v2.a;
								var row = _v2.b;
								var y = gridOriginY + (row * spacingY);
								var x = gridOriginX + (col * spacingX);
								var tileDelay = placementStep(index) * tileInterval;
								var overlays = getScoringOverlays(index);
								var _v3 = getTileCode(index);
								if (_v3.$ === 'Just') {
									var tileCode = _v3.a;
									var _v4 = $author$project$TileSvg$parseTileFromPath(tileCode);
									if (_v4.$ === 'Just') {
										var tileData = _v4.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('hex-cell filled welcome-tile'),
													A2(
													$elm$html$Html$Attributes$style,
													'left',
													$elm$core$String$fromFloat(x) + 'px'),
													A2(
													$elm$html$Html$Attributes$style,
													'top',
													$elm$core$String$fromFloat(y) + 'px'),
													A2(
													$elm$html$Html$Attributes$style,
													'width',
													$elm$core$String$fromFloat(hexWidth) + 'px'),
													A2(
													$elm$html$Html$Attributes$style,
													'height',
													$elm$core$String$fromFloat(hexHeight) + 'px'),
													A2(
													$elm$html$Html$Attributes$style,
													'animation-delay',
													$elm$core$String$fromFloat(tileDelay) + 's')
												]),
											A2(
												$elm$core$List$cons,
												A2(
													$elm$html$Html$div,
													_List_fromArray(
														[
															$elm$html$Html$Attributes$class('hex-tile-svg')
														]),
													_List_fromArray(
														[
															$author$project$TileSvg$viewTileSvg(tileData)
														])),
												A2($elm$core$List$map, viewScoringOverlay, overlays)));
									} else {
										return $elm$html$Html$text('');
									}
								} else {
									return $elm$html$Html$text('');
								}
							}),
						hexPositions),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('score-label phase-v1'),
									A2($elm$html$Html$Attributes$style, 'animation-delay', '8.5s'),
									A2($elm$html$Html$Attributes$style, 'left', '-22px'),
									A2($elm$html$Html$Attributes$style, 'top', '158px')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('27')
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('score-label phase-v1'),
									A2($elm$html$Html$Attributes$style, 'animation-delay', '8.5s'),
									A2($elm$html$Html$Attributes$style, 'left', '310px'),
									A2($elm$html$Html$Attributes$style, 'top', '158px')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('15')
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('score-label phase-v2'),
									A2($elm$html$Html$Attributes$style, 'animation-delay', '11.0s'),
									A2($elm$html$Html$Attributes$style, 'left', '166px'),
									A2($elm$html$Html$Attributes$style, 'top', '8px')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('18')
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('score-label phase-v2'),
									A2($elm$html$Html$Attributes$style, 'animation-delay', '11.0s'),
									A2($elm$html$Html$Attributes$style, 'left', '310px'),
									A2($elm$html$Html$Attributes$style, 'top', '218px')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('21')
								])),
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('score-label phase-v3'),
									A2($elm$html$Html$Attributes$style, 'animation-delay', '13.5s'),
									A2($elm$html$Html$Attributes$style, 'left', '166px'),
									A2($elm$html$Html$Attributes$style, 'top', '305px')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('24')
								]))
						]))),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-score'),
						A2($elm$html$Html$Attributes$style, 'animation-delay', '16.0s')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Score : 105 pts')
					]))
			]));
}();
var $author$project$Main$viewWelcome = function (_v0) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('welcome-content')
			]),
		_List_fromArray(
			[
				$author$project$Main$viewWelcomeBoard,
				A2(
				$elm$html$Html$p,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-pitch')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Placez vos tuiles, marquez des points et défiez l\'IA ou vos amis !')
					])),
				A2(
				$elm$html$Html$button,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-play-button'),
						$elm$html$Html$Events$onClick($author$project$Main$SkipAuth)
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('Jouer maintenant')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-separator')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text('ou')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('welcome-links')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('button'),
								$elm$html$Html$Attributes$class('link-button'),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SwitchAuthView($author$project$Main$Login))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Se connecter')
							])),
						$elm$html$Html$text(' · '),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('button'),
								$elm$html$Html$Attributes$class('link-button'),
								$elm$html$Html$Events$onClick(
								$author$project$Main$SwitchAuthView($author$project$Main$Register))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Créer un compte')
							]))
					]))
			]));
};
var $author$project$Main$viewAuth = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('auth-page')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('auth-container glass-container')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('auth-header')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$h1,
								_List_Nil,
								_List_fromArray(
									[
										$elm$html$Html$text('Take It Easy')
									])),
								A2(
								$elm$html$Html$p,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('auth-subtitle')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text(
										$author$project$Main$authSubtitle(model.authView))
									]))
							])),
						(model.authError !== '') ? A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('auth-error')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(model.authError)
							])) : ((model.resetMessage !== '') ? A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('auth-success')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(model.resetMessage)
							])) : $elm$html$Html$text('')),
						function () {
						var _v0 = model.authView;
						switch (_v0.$) {
							case 'Welcome':
								return $author$project$Main$viewWelcome(model);
							case 'ForgotPassword':
								return $author$project$Main$viewForgotPasswordForm(model);
							case 'ResetPassword':
								return $author$project$Main$viewResetPasswordForm(model);
							default:
								return $author$project$Main$viewLoginRegisterForm(model);
						}
					}(),
						$author$project$Main$viewAuthFooter(model)
					]))
			]));
};
var $author$project$Main$BackToModeSelection = {$: 'BackToModeSelection'};
var $author$project$Main$CreateSession = {$: 'CreateSession'};
var $author$project$Main$JoinSession = {$: 'JoinSession'};
var $author$project$Main$SetPlayerName = function (a) {
	return {$: 'SetPlayerName', a: a};
};
var $author$project$Main$SetSessionCode = function (a) {
	return {$: 'SetSessionCode', a: a};
};
var $elm$html$Html$h2 = _VirtualDom_node('h2');
var $elm$html$Html$h3 = _VirtualDom_node('h3');
var $author$project$Main$viewConnectionInterface = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('connection-interface glass-container')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$h2,
				_List_Nil,
				_List_fromArray(
					[
						$elm$html$Html$text('Connexion à une partie')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('form-group')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$label,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$for('playerName')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Votre nom')
							])),
						A2(
						$elm$html$Html$input,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$type_('text'),
								$elm$html$Html$Attributes$id('playerName'),
								$elm$html$Html$Attributes$value(model.playerName),
								$elm$html$Html$Events$onInput($author$project$Main$SetPlayerName),
								$elm$html$Html$Attributes$placeholder('Entrez votre nom'),
								$elm$html$Html$Attributes$disabled(model.loading)
							]),
						_List_Nil)
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('connection-buttons')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('create-button'),
								$elm$html$Html$Events$onClick($author$project$Main$CreateSession),
								$elm$html$Html$Attributes$disabled(model.loading || (model.playerName === ''))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Créer une partie')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('join-section')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$h3,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Ou rejoindre une partie')
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('form-group')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$input,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$type_('text'),
										$elm$html$Html$Attributes$value(model.sessionCode),
										$elm$html$Html$Events$onInput($author$project$Main$SetSessionCode),
										$elm$html$Html$Attributes$placeholder('Code de session'),
										$elm$html$Html$Attributes$disabled(model.loading)
									]),
								_List_Nil)
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('join-button'),
								$elm$html$Html$Events$onClick($author$project$Main$JoinSession),
								$elm$html$Html$Attributes$disabled(model.loading || ((model.playerName === '') || (model.sessionCode === '')))
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Rejoindre')
							]))
					]))
			]));
};
var $author$project$Main$LeaveSession = {$: 'LeaveSession'};
var $elm$html$Html$ul = _VirtualDom_node('ul');
var $author$project$Main$RestartSoloGame = {$: 'RestartSoloGame'};
var $elm$html$Html$li = _VirtualDom_node('li');
var $elm$core$Basics$negate = function (n) {
	return -n;
};
var $elm$core$List$sortBy = _List_sortBy;
var $elm$core$List$drop = F2(
	function (n, list) {
		drop:
		while (true) {
			if (n <= 0) {
				return list;
			} else {
				if (!list.b) {
					return list;
				} else {
					var x = list.a;
					var xs = list.b;
					var $temp$n = n - 1,
						$temp$list = xs;
					n = $temp$n;
					list = $temp$list;
					continue drop;
				}
			}
		}
	});
var $elm$html$Html$img = _VirtualDom_node('img');
var $elm$html$Html$Attributes$src = function (url) {
	return A2(
		$elm$html$Html$Attributes$stringProperty,
		'src',
		_VirtualDom_noJavaScriptOrHtmlUri(url));
};
var $author$project$TileSvg$viewEmptyHexSvg = F2(
	function (isAvailable, index) {
		var strokeColor = isAvailable ? '#4ecdc4' : '#444';
		var hexPoints = '25,0 75,0 100,43.3 75,86.6 25,86.6 0,43.3';
		var fillColor = isAvailable ? 'rgba(78, 205, 196, 0.3)' : '#1a1a2e';
		return A2(
			$elm$svg$Svg$svg,
			_List_fromArray(
				[
					$elm$svg$Svg$Attributes$viewBox('0 0 100 86.6'),
					$elm$svg$Svg$Attributes$width('100%'),
					$elm$svg$Svg$Attributes$height('100%')
				]),
			_List_fromArray(
				[
					A2(
					$elm$svg$Svg$polygon,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$points(hexPoints),
							$elm$svg$Svg$Attributes$fill(fillColor),
							$elm$svg$Svg$Attributes$stroke(strokeColor),
							$elm$svg$Svg$Attributes$strokeWidth('2')
						]),
					_List_Nil),
					A2(
					$elm$svg$Svg$text_,
					_List_fromArray(
						[
							$elm$svg$Svg$Attributes$x('50'),
							$elm$svg$Svg$Attributes$y('43.3'),
							$elm$svg$Svg$Attributes$textAnchor('middle'),
							$elm$svg$Svg$Attributes$dominantBaseline('middle'),
							$elm$svg$Svg$Attributes$fontSize('14'),
							$elm$svg$Svg$Attributes$fill('rgba(255, 255, 255, 0.5)')
						]),
					_List_fromArray(
						[
							$elm$svg$Svg$text(
							$elm$core$String$fromInt(index))
						]))
				]));
	});
var $author$project$Main$viewFinalHexBoard = function (tiles) {
	var hexRadius = 36;
	var hexWidth = 2 * hexRadius;
	var spacingX = 0.75 * hexWidth;
	var hexPositions = _List_fromArray(
		[
			_Utils_Tuple2(0, 1),
			_Utils_Tuple2(0, 2),
			_Utils_Tuple2(0, 3),
			_Utils_Tuple2(1, 0.5),
			_Utils_Tuple2(1, 1.5),
			_Utils_Tuple2(1, 2.5),
			_Utils_Tuple2(1, 3.5),
			_Utils_Tuple2(2, 0),
			_Utils_Tuple2(2, 1),
			_Utils_Tuple2(2, 2),
			_Utils_Tuple2(2, 3),
			_Utils_Tuple2(2, 4),
			_Utils_Tuple2(3, 0.5),
			_Utils_Tuple2(3, 1.5),
			_Utils_Tuple2(3, 2.5),
			_Utils_Tuple2(3, 3.5),
			_Utils_Tuple2(4, 1),
			_Utils_Tuple2(4, 2),
			_Utils_Tuple2(4, 3)
		]);
	var hexHeight = 1.732 * hexRadius;
	var spacingY = hexHeight;
	var gridOriginY = 20;
	var gridOriginX = 16;
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('hex-board final-hex-board'),
				A2($elm$html$Html$Attributes$style, 'position', 'relative'),
				A2($elm$html$Html$Attributes$style, 'width', '320px'),
				A2($elm$html$Html$Attributes$style, 'height', '340px'),
				A2($elm$html$Html$Attributes$style, 'margin', '0 auto')
			]),
		A2(
			$elm$core$List$indexedMap,
			F2(
				function (index, _v0) {
					var col = _v0.a;
					var row = _v0.b;
					var y = gridOriginY + (row * spacingY);
					var x = gridOriginX + (col * spacingX);
					var tile = A2(
						$elm$core$Maybe$withDefault,
						'',
						$elm$core$List$head(
							A2($elm$core$List$drop, index, tiles)));
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class(
								'hex-cell' + ((tile !== '') ? ' filled' : '')),
								A2(
								$elm$html$Html$Attributes$style,
								'left',
								$elm$core$String$fromFloat(x) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'top',
								$elm$core$String$fromFloat(y) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'width',
								$elm$core$String$fromFloat(hexWidth) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'height',
								$elm$core$String$fromFloat(hexHeight) + 'px')
							]),
						_List_fromArray(
							[
								function () {
								if (tile !== '') {
									var _v1 = $author$project$TileSvg$parseTileFromPath(tile);
									if (_v1.$ === 'Just') {
										var tileData = _v1.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('hex-tile-svg')
												]),
											_List_fromArray(
												[
													$author$project$TileSvg$viewTileSvg(tileData)
												]));
									} else {
										return A2(
											$elm$html$Html$img,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$src(tile),
													$elm$html$Html$Attributes$class('hex-tile-image')
												]),
											_List_Nil);
									}
								} else {
									return A2($author$project$TileSvg$viewEmptyHexSvg, false, index);
								}
							}()
							]));
				}),
			hexPositions));
};
var $author$project$Main$viewFinishedState = F2(
	function (model, gameState) {
		var sortedPlayers = A2(
			$elm$core$List$sortBy,
			function (p) {
				return -p.score;
			},
			gameState.players);
		var winner = $elm$core$List$head(sortedPlayers);
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('finished-state')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('finished-header glass-container')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$h2,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('🎉 Partie terminée!')
								])),
							function () {
							if (winner.$ === 'Just') {
								var w = winner.a;
								return A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('winner')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											'🏆 Gagnant: ' + (w.name + (' avec ' + ($elm$core$String$fromInt(w.score) + ' points!'))))
										]));
							} else {
								return $elm$html$Html$text('');
							}
						}()
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('finished-content')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('final-scores glass-container')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$h3,
									_List_Nil,
									_List_fromArray(
										[
											$elm$html$Html$text('Classement final')
										])),
									A2(
									$elm$html$Html$ul,
									_List_Nil,
									A2(
										$elm$core$List$indexedMap,
										F2(
											function (i, p) {
												return A2(
													$elm$html$Html$li,
													_List_fromArray(
														[
															$elm$html$Html$Attributes$class('final-score-item')
														]),
													_List_fromArray(
														[
															A2(
															$elm$html$Html$span,
															_List_fromArray(
																[
																	$elm$html$Html$Attributes$class('rank')
																]),
															_List_fromArray(
																[
																	$elm$html$Html$text(
																	$elm$core$String$fromInt(i + 1) + '.')
																])),
															A2(
															$elm$html$Html$span,
															_List_fromArray(
																[
																	$elm$html$Html$Attributes$class('name')
																]),
															_List_fromArray(
																[
																	$elm$html$Html$text(
																	(p.id === 'mcts_ai') ? '🤖 IA' : ('👤 ' + p.name))
																])),
															A2(
															$elm$html$Html$span,
															_List_fromArray(
																[
																	$elm$html$Html$Attributes$class('score')
																]),
															_List_fromArray(
																[
																	$elm$html$Html$text(
																	$elm$core$String$fromInt(p.score) + ' pts')
																]))
														]));
											}),
										sortedPlayers))
								]))
						])),
					function () {
					if ($elm$core$List$length(model.allPlayerPlateaus) > 2) {
						var myId = A2(
							$elm$core$Maybe$withDefault,
							'',
							A2(
								$elm$core$Maybe$map,
								function ($) {
									return $.playerId;
								},
								model.session));
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('finished-boards')
								]),
							A2(
								$elm$core$List$map,
								function (_v1) {
									var pid = _v1.a;
									var pname = _v1.b;
									var tiles = _v1.c;
									var isMe = _Utils_eq(pid, myId);
									var isAi = pid === 'mcts_ai';
									var label = isAi ? ('🤖 ' + pname) : (isMe ? ('👤 ' + (pname + ' (vous)')) : ('👤 ' + pname));
									var boardClass = 'final-board glass-container' + (isMe ? ' current-player-board' : '');
									return A2(
										$elm$html$Html$div,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class(boardClass)
											]),
										_List_fromArray(
											[
												A2(
												$elm$html$Html$h3,
												_List_Nil,
												_List_fromArray(
													[
														$elm$html$Html$text(label)
													])),
												$author$project$Main$viewFinalHexBoard(tiles)
											]));
								},
								model.allPlayerPlateaus));
					} else {
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('finished-boards')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('final-board glass-container')
										]),
									_List_fromArray(
										[
											A2(
											$elm$html$Html$h3,
											_List_Nil,
											_List_fromArray(
												[
													$elm$html$Html$text('👤 Votre plateau')
												])),
											$author$project$Main$viewFinalHexBoard(model.plateauTiles)
										])),
									A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('final-board glass-container')
										]),
									_List_fromArray(
										[
											A2(
											$elm$html$Html$h3,
											_List_Nil,
											_List_fromArray(
												[
													$elm$html$Html$text('🤖 Plateau IA')
												])),
											$author$project$Main$viewFinalHexBoard(model.aiPlateauTiles)
										]))
								]));
					}
				}(),
					model.isSoloMode ? A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('play-again-button'),
							$elm$html$Html$Events$onClick($author$project$Main$RestartSoloGame)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('🔄 Rejouer')
						])) : A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('play-again-button'),
							$elm$html$Html$Events$onClick($author$project$Main$BackToModeSelection)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('Rejouer')
						]))
				]));
	});
var $author$project$Main$StartTurn = {$: 'StartTurn'};
var $author$project$Main$ToggleAiBoard = {$: 'ToggleAiBoard'};
var $author$project$Main$viewAiHexBoard = function (tiles) {
	var hexRadius = 40;
	var hexWidth = 2 * hexRadius;
	var spacingX = 0.75 * hexWidth;
	var hexPositions = _List_fromArray(
		[
			_Utils_Tuple2(0, 1),
			_Utils_Tuple2(0, 2),
			_Utils_Tuple2(0, 3),
			_Utils_Tuple2(1, 0.5),
			_Utils_Tuple2(1, 1.5),
			_Utils_Tuple2(1, 2.5),
			_Utils_Tuple2(1, 3.5),
			_Utils_Tuple2(2, 0),
			_Utils_Tuple2(2, 1),
			_Utils_Tuple2(2, 2),
			_Utils_Tuple2(2, 3),
			_Utils_Tuple2(2, 4),
			_Utils_Tuple2(3, 0.5),
			_Utils_Tuple2(3, 1.5),
			_Utils_Tuple2(3, 2.5),
			_Utils_Tuple2(3, 3.5),
			_Utils_Tuple2(4, 1),
			_Utils_Tuple2(4, 2),
			_Utils_Tuple2(4, 3)
		]);
	var hexHeight = 1.732 * hexRadius;
	var spacingY = hexHeight;
	var gridOriginY = 17;
	var gridOriginX = 10;
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('hex-board ai-board'),
				A2($elm$html$Html$Attributes$style, 'position', 'relative'),
				A2($elm$html$Html$Attributes$style, 'width', '340px'),
				A2($elm$html$Html$Attributes$style, 'height', '380px'),
				A2($elm$html$Html$Attributes$style, 'margin', '0 auto')
			]),
		A2(
			$elm$core$List$indexedMap,
			F2(
				function (index, _v0) {
					var col = _v0.a;
					var row = _v0.b;
					var y = gridOriginY + (row * spacingY);
					var x = gridOriginX + (col * spacingX);
					var tile = A2(
						$elm$core$Maybe$withDefault,
						'',
						$elm$core$List$head(
							A2($elm$core$List$drop, index, tiles)));
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class(
								'hex-cell' + ((tile !== '') ? ' filled' : '')),
								A2(
								$elm$html$Html$Attributes$style,
								'left',
								$elm$core$String$fromFloat(x) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'top',
								$elm$core$String$fromFloat(y) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'width',
								$elm$core$String$fromFloat(hexWidth) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'height',
								$elm$core$String$fromFloat(hexHeight) + 'px')
							]),
						_List_fromArray(
							[
								function () {
								if (tile !== '') {
									var _v1 = $author$project$TileSvg$parseTileFromPath(tile);
									if (_v1.$ === 'Just') {
										var tileData = _v1.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('hex-tile-svg')
												]),
											_List_fromArray(
												[
													$author$project$TileSvg$viewTileSvg(tileData)
												]));
									} else {
										return A2(
											$elm$html$Html$img,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$src(tile),
													$elm$html$Html$Attributes$class('hex-tile-image')
												]),
											_List_Nil);
									}
								} else {
									return A2($author$project$TileSvg$viewEmptyHexSvg, false, index);
								}
							}()
							]));
				}),
			hexPositions));
};
var $author$project$Main$PlayMove = function (a) {
	return {$: 'PlayMove', a: a};
};
var $author$project$Main$viewHexBoard = function (model) {
	var hexRadius = 60;
	var hexWidth = 2 * hexRadius;
	var spacingX = 0.75 * hexWidth;
	var hexPositions = _List_fromArray(
		[
			_Utils_Tuple2(0, 1),
			_Utils_Tuple2(0, 2),
			_Utils_Tuple2(0, 3),
			_Utils_Tuple2(1, 0.5),
			_Utils_Tuple2(1, 1.5),
			_Utils_Tuple2(1, 2.5),
			_Utils_Tuple2(1, 3.5),
			_Utils_Tuple2(2, 0),
			_Utils_Tuple2(2, 1),
			_Utils_Tuple2(2, 2),
			_Utils_Tuple2(2, 3),
			_Utils_Tuple2(2, 4),
			_Utils_Tuple2(3, 0.5),
			_Utils_Tuple2(3, 1.5),
			_Utils_Tuple2(3, 2.5),
			_Utils_Tuple2(3, 3.5),
			_Utils_Tuple2(4, 1),
			_Utils_Tuple2(4, 2),
			_Utils_Tuple2(4, 3)
		]);
	var hexHeight = 1.732 * hexRadius;
	var spacingY = hexHeight;
	var gridOriginY = 25;
	var gridOriginX = 55;
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('hex-board'),
				A2($elm$html$Html$Attributes$style, 'position', 'relative'),
				A2($elm$html$Html$Attributes$style, 'width', '600px'),
				A2($elm$html$Html$Attributes$style, 'height', '570px'),
				A2($elm$html$Html$Attributes$style, 'margin', '0 auto')
			]),
		A2(
			$elm$core$List$indexedMap,
			F2(
				function (index, _v0) {
					var col = _v0.a;
					var row = _v0.b;
					var y = gridOriginY + (row * spacingY);
					var x = gridOriginX + (col * spacingX);
					var tile = A2(
						$elm$core$Maybe$withDefault,
						'',
						$elm$core$List$head(
							A2($elm$core$List$drop, index, model.plateauTiles)));
					var isAvailable = A2($elm$core$List$member, index, model.availablePositions) && model.myTurn;
					var canClick = isAvailable && (!_Utils_eq(model.currentTile, $elm$core$Maybe$Nothing));
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class(
								'hex-cell' + ((isAvailable ? ' available' : '') + ((tile !== '') ? ' filled' : ''))),
								A2(
								$elm$html$Html$Attributes$style,
								'left',
								$elm$core$String$fromFloat(x) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'top',
								$elm$core$String$fromFloat(y) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'width',
								$elm$core$String$fromFloat(hexWidth) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'height',
								$elm$core$String$fromFloat(hexHeight) + 'px'),
								canClick ? $elm$html$Html$Events$onClick(
								$author$project$Main$PlayMove(index)) : $elm$html$Html$Attributes$class('')
							]),
						_List_fromArray(
							[
								function () {
								if (tile !== '') {
									var _v1 = $author$project$TileSvg$parseTileFromPath(tile);
									if (_v1.$ === 'Just') {
										var tileData = _v1.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('hex-tile-svg')
												]),
											_List_fromArray(
												[
													$author$project$TileSvg$viewTileSvg(tileData)
												]));
									} else {
										return A2(
											$elm$html$Html$img,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$src(tile),
													$elm$html$Html$Attributes$class('hex-tile-image')
												]),
											_List_Nil);
									}
								} else {
									return A2($author$project$TileSvg$viewEmptyHexSvg, isAvailable, index);
								}
							}()
							]));
				}),
			hexPositions));
};
var $author$project$Main$viewInProgressState = F2(
	function (model, session) {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('in-progress-state')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('turn-info glass-container')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$h3,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text(
									'Tour ' + ($elm$core$String$fromInt(model.currentTurnNumber) + '/19'))
								])),
							function () {
							var _v0 = model.currentTile;
							if (_v0.$ === 'Just') {
								return A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('current-tile')
										]),
									_List_fromArray(
										[
											function () {
											var _v1 = model.currentTileImage;
											if (_v1.$ === 'Just') {
												var img = _v1.a;
												var _v2 = $author$project$TileSvg$parseTileFromPath(img);
												if (_v2.$ === 'Just') {
													var tileData = _v2.a;
													return A2(
														$elm$html$Html$div,
														_List_fromArray(
															[
																$elm$html$Html$Attributes$class('tile-svg-container')
															]),
														_List_fromArray(
															[
																$author$project$TileSvg$viewTileSvg(tileData)
															]));
												} else {
													return A2(
														$elm$html$Html$img,
														_List_fromArray(
															[
																$elm$html$Html$Attributes$src(img),
																$elm$html$Html$Attributes$class('tile-image')
															]),
														_List_Nil);
												}
											} else {
												return $elm$html$Html$text('');
											}
										}()
										]));
							} else {
								return ((!model.myTurn) && (!$elm$core$List$isEmpty(model.waitingForPlayers))) ? A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('waiting-message')
										]),
									_List_fromArray(
										[
											A2(
											$elm$html$Html$p,
											_List_fromArray(
												[
													A2($elm$html$Html$Attributes$style, 'opacity', '0.8')
												]),
											_List_fromArray(
												[
													$elm$html$Html$text(
													'En attente de ' + ($elm$core$String$fromInt(
														$elm$core$List$length(model.waitingForPlayers)) + ' joueur(s)...'))
												]))
										])) : A2(
									$elm$html$Html$button,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('start-turn-button'),
											$elm$html$Html$Events$onClick($author$project$Main$StartTurn),
											$elm$html$Html$Attributes$disabled(model.loading)
										]),
									_List_fromArray(
										[
											$elm$html$Html$text('Commencer le tour')
										]));
							}
						}()
						])),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('game-board glass-container')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$h3,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('Plateau de jeu')
								])),
							$author$project$Main$viewHexBoard(model),
							model.isSoloMode ? A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									A2($elm$html$Html$Attributes$style, 'margin-top', '15px'),
									A2($elm$html$Html$Attributes$style, 'text-align', 'center')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$button,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('toggle-ai-board-button'),
											$elm$html$Html$Events$onClick($author$project$Main$ToggleAiBoard),
											A2($elm$html$Html$Attributes$style, 'padding', '8px 16px'),
											A2($elm$html$Html$Attributes$style, 'border-radius', '8px'),
											A2($elm$html$Html$Attributes$style, 'border', 'none'),
											A2($elm$html$Html$Attributes$style, 'background', 'rgba(255,255,255,0.2)'),
											A2($elm$html$Html$Attributes$style, 'cursor', 'pointer')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text(
											model.showAiBoard ? '🤖 Masquer plateau IA' : '🤖 Voir plateau IA')
										]))
								])) : $elm$html$Html$text('')
						])),
					(model.isSoloMode && model.showAiBoard) ? A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('game-board glass-container'),
							A2($elm$html$Html$Attributes$style, 'margin-top', '20px')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$h3,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text(
									'🤖 Plateau IA - ' + ($elm$core$String$fromInt(model.aiScore) + ' pts'))
								])),
							$author$project$Main$viewAiHexBoard(model.aiPlateauTiles)
						])) : $elm$html$Html$text('')
				]));
	});
var $author$project$Main$viewPlayer = F2(
	function (myPlayerId, player) {
		return A2(
			$elm$html$Html$li,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class(
					'player-item' + (_Utils_eq(player.id, myPlayerId) ? ' self' : ((player.id === 'mcts_ai') ? ' ai' : '')))
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$span,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('player-name')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(
							(player.id === 'mcts_ai') ? '🤖 IA' : ('👤 ' + player.name))
						])),
					A2(
					$elm$html$Html$span,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('player-score')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(
							$elm$core$String$fromInt(player.score) + ' pts')
						])),
					player.isReady ? A2(
					$elm$html$Html$span,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('ready-badge')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('✓')
						])) : $elm$html$Html$text('')
				]));
	});
var $author$project$Main$SetReady = {$: 'SetReady'};
var $author$project$Main$viewWaitingState = F3(
	function (model, session, gameState) {
		var currentPlayer = $elm$core$List$head(
			A2(
				$elm$core$List$filter,
				function (p) {
					return _Utils_eq(p.id, session.playerId);
				},
				gameState.players));
		var isReady = A2(
			$elm$core$Maybe$withDefault,
			false,
			A2(
				$elm$core$Maybe$map,
				function ($) {
					return $.isReady;
				},
				currentPlayer));
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('waiting-state glass-container')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$h3,
					_List_Nil,
					_List_fromArray(
						[
							$elm$html$Html$text('En attente des joueurs')
						])),
					isReady ? A2(
					$elm$html$Html$p,
					_List_Nil,
					_List_fromArray(
						[
							$elm$html$Html$text('Vous êtes prêt! En attente des autres joueurs...')
						])) : A2(
					$elm$html$Html$button,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('ready-button'),
							$elm$html$Html$Events$onClick($author$project$Main$SetReady),
							$elm$html$Html$Attributes$disabled(model.loading)
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('Je suis prêt!')
						]))
				]));
	});
var $author$project$Main$viewGameState = F3(
	function (model, gameState, session) {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('game-state')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('players-list glass-container')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$h3,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('Joueurs')
								])),
							A2(
							$elm$html$Html$ul,
							_List_Nil,
							A2(
								$elm$core$List$map,
								$author$project$Main$viewPlayer(session.playerId),
								gameState.players))
						])),
					function () {
					var _v0 = gameState.state;
					switch (_v0.$) {
						case 'Waiting':
							return A3($author$project$Main$viewWaitingState, model, session, gameState);
						case 'InProgress':
							return A2($author$project$Main$viewInProgressState, model, session);
						case 'Finished':
							return A2($author$project$Main$viewFinishedState, model, gameState);
						default:
							return A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('cancelled')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('Partie annulée')
									]));
					}
				}()
				]));
	});
var $author$project$Main$viewGameSession = F2(
	function (model, session) {
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class('game-session')
				]),
			_List_fromArray(
				[
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('session-info glass-container')
						]),
					_List_fromArray(
						[
							A2(
							$elm$html$Html$h2,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('Session: ' + session.sessionCode)
								])),
							A2(
							$elm$html$Html$p,
							_List_Nil,
							_List_fromArray(
								[
									$elm$html$Html$text('Joueur: ' + model.playerName)
								])),
							A2(
							$elm$html$Html$button,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('leave-button'),
									$elm$html$Html$Events$onClick($author$project$Main$LeaveSession)
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('Quitter')
								]))
						])),
					function () {
					var _v0 = model.gameState;
					if (_v0.$ === 'Just') {
						var gameState = _v0.a;
						return A3($author$project$Main$viewGameState, model, gameState, session);
					} else {
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('loading')
								]),
							_List_fromArray(
								[
									$elm$html$Html$text('Chargement...')
								]));
					}
				}()
				]));
	});
var $author$project$Main$ResetRealGame = {$: 'ResetRealGame'};
var $elm$core$Basics$composeL = F3(
	function (g, f, x) {
		return g(
			f(x));
	});
var $elm$core$List$all = F2(
	function (isOkay, list) {
		return !A2(
			$elm$core$List$any,
			A2($elm$core$Basics$composeL, $elm$core$Basics$not, isOkay),
			list);
	});
var $elm$core$Maybe$andThen = F2(
	function (callback, maybeValue) {
		if (maybeValue.$ === 'Just') {
			var value = maybeValue.a;
			return callback(value);
		} else {
			return $elm$core$Maybe$Nothing;
		}
	});
var $elm$core$List$sum = function (numbers) {
	return A3($elm$core$List$foldl, $elm$core$Basics$add, 0, numbers);
};
var $author$project$Main$calculateBoardScore = function (tiles) {
	var v3Lines = _List_fromArray(
		[
			_List_fromArray(
			[0, 3, 7]),
			_List_fromArray(
			[1, 4, 8, 12]),
			_List_fromArray(
			[2, 5, 9, 13, 16]),
			_List_fromArray(
			[6, 10, 14, 17]),
			_List_fromArray(
			[11, 15, 18])
		]);
	var v2Lines = _List_fromArray(
		[
			_List_fromArray(
			[7, 12, 16]),
			_List_fromArray(
			[3, 8, 13, 17]),
			_List_fromArray(
			[0, 4, 9, 14, 18]),
			_List_fromArray(
			[1, 5, 10, 15]),
			_List_fromArray(
			[2, 6, 11])
		]);
	var v1Lines = _List_fromArray(
		[
			_List_fromArray(
			[0, 1, 2]),
			_List_fromArray(
			[3, 4, 5, 6]),
			_List_fromArray(
			[7, 8, 9, 10, 11]),
			_List_fromArray(
			[12, 13, 14, 15]),
			_List_fromArray(
			[16, 17, 18])
		]);
	var parsedTiles = A2(
		$elm$core$List$indexedMap,
		F2(
			function (i, t) {
				return _Utils_Tuple2(
					i,
					$author$project$TileSvg$parseTileFromPath(t));
			}),
		tiles);
	var tileAt = function (pos) {
		return A2(
			$elm$core$Maybe$andThen,
			$elm$core$Tuple$second,
			$elm$core$List$head(
				A2(
					$elm$core$List$filter,
					function (_v1) {
						var i = _v1.a;
						return _Utils_eq(i, pos);
					},
					parsedTiles)));
	};
	var scoreLine = F2(
		function (getValue, positions) {
			var values = A2(
				$elm$core$List$filterMap,
				function (pos) {
					return A2(
						$elm$core$Maybe$map,
						getValue,
						tileAt(pos));
				},
				positions);
			if (_Utils_eq(
				$elm$core$List$length(values),
				$elm$core$List$length(positions))) {
				if (values.b) {
					var first = values.a;
					var rest = values.b;
					return A2(
						$elm$core$List$all,
						function (v) {
							return _Utils_eq(v, first);
						},
						rest) ? (first * $elm$core$List$length(positions)) : 0;
				} else {
					return 0;
				}
			} else {
				return 0;
			}
		});
	return ($elm$core$List$sum(
		A2(
			$elm$core$List$map,
			scoreLine(
				function ($) {
					return $.v1;
				}),
			v1Lines)) + $elm$core$List$sum(
		A2(
			$elm$core$List$map,
			scoreLine(
				function ($) {
					return $.v2;
				}),
			v2Lines))) + $elm$core$List$sum(
		A2(
			$elm$core$List$map,
			scoreLine(
				function ($) {
					return $.v3;
				}),
			v3Lines));
};
var $author$project$Main$viewAiRealGameBoard = function (model) {
	var hexRadius = 45;
	var hexWidth = 2 * hexRadius;
	var spacingX = 0.75 * hexWidth;
	var hexPositions = _List_fromArray(
		[
			_Utils_Tuple2(0, 1),
			_Utils_Tuple2(0, 2),
			_Utils_Tuple2(0, 3),
			_Utils_Tuple2(1, 0.5),
			_Utils_Tuple2(1, 1.5),
			_Utils_Tuple2(1, 2.5),
			_Utils_Tuple2(1, 3.5),
			_Utils_Tuple2(2, 0),
			_Utils_Tuple2(2, 1),
			_Utils_Tuple2(2, 2),
			_Utils_Tuple2(2, 3),
			_Utils_Tuple2(2, 4),
			_Utils_Tuple2(3, 0.5),
			_Utils_Tuple2(3, 1.5),
			_Utils_Tuple2(3, 2.5),
			_Utils_Tuple2(3, 3.5),
			_Utils_Tuple2(4, 1),
			_Utils_Tuple2(4, 2),
			_Utils_Tuple2(4, 3)
		]);
	var hexHeight = 1.732 * hexRadius;
	var spacingY = hexHeight;
	var gridOriginY = 20;
	var gridOriginX = 45;
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('hex-board ai-hex-board'),
				A2($elm$html$Html$Attributes$style, 'position', 'relative'),
				A2($elm$html$Html$Attributes$style, 'width', '450px'),
				A2($elm$html$Html$Attributes$style, 'height', '430px'),
				A2($elm$html$Html$Attributes$style, 'margin', '0 auto')
			]),
		A2(
			$elm$core$List$indexedMap,
			F2(
				function (index, _v0) {
					var col = _v0.a;
					var row = _v0.b;
					var y = gridOriginY + (row * spacingY);
					var x = gridOriginX + (col * spacingX);
					var tile = A2(
						$elm$core$Maybe$withDefault,
						'',
						$elm$core$List$head(
							A2($elm$core$List$drop, index, model.aiPlateauTiles)));
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class(
								'hex-cell' + ((tile !== '') ? ' filled' : '')),
								A2(
								$elm$html$Html$Attributes$style,
								'left',
								$elm$core$String$fromFloat(x) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'top',
								$elm$core$String$fromFloat(y) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'width',
								$elm$core$String$fromFloat(hexWidth) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'height',
								$elm$core$String$fromFloat(hexHeight) + 'px')
							]),
						_List_fromArray(
							[
								function () {
								if (tile !== '') {
									var _v1 = $author$project$TileSvg$parseTileFromPath(tile);
									if (_v1.$ === 'Just') {
										var tileData = _v1.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('hex-tile-svg')
												]),
											_List_fromArray(
												[
													$author$project$TileSvg$viewTileSvg(tileData)
												]));
									} else {
										return $elm$html$Html$text('');
									}
								} else {
									return A2($author$project$TileSvg$viewEmptyHexSvg, false, index);
								}
							}()
							]));
				}),
			hexPositions));
};
var $author$project$Main$PlaceRealTile = function (a) {
	return {$: 'PlaceRealTile', a: a};
};
var $author$project$Main$viewRealGameBoard = function (model) {
	var hexRadius = 45;
	var hexWidth = 2 * hexRadius;
	var spacingX = 0.75 * hexWidth;
	var hexPositions = _List_fromArray(
		[
			_Utils_Tuple2(0, 1),
			_Utils_Tuple2(0, 2),
			_Utils_Tuple2(0, 3),
			_Utils_Tuple2(1, 0.5),
			_Utils_Tuple2(1, 1.5),
			_Utils_Tuple2(1, 2.5),
			_Utils_Tuple2(1, 3.5),
			_Utils_Tuple2(2, 0),
			_Utils_Tuple2(2, 1),
			_Utils_Tuple2(2, 2),
			_Utils_Tuple2(2, 3),
			_Utils_Tuple2(2, 4),
			_Utils_Tuple2(3, 0.5),
			_Utils_Tuple2(3, 1.5),
			_Utils_Tuple2(3, 2.5),
			_Utils_Tuple2(3, 3.5),
			_Utils_Tuple2(4, 1),
			_Utils_Tuple2(4, 2),
			_Utils_Tuple2(4, 3)
		]);
	var hexHeight = 1.732 * hexRadius;
	var spacingY = hexHeight;
	var gridOriginY = 20;
	var gridOriginX = 45;
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('hex-board'),
				A2($elm$html$Html$Attributes$style, 'position', 'relative'),
				A2($elm$html$Html$Attributes$style, 'width', '450px'),
				A2($elm$html$Html$Attributes$style, 'height', '430px'),
				A2($elm$html$Html$Attributes$style, 'margin', '0 auto')
			]),
		A2(
			$elm$core$List$indexedMap,
			F2(
				function (index, _v0) {
					var col = _v0.a;
					var row = _v0.b;
					var y = gridOriginY + (row * spacingY);
					var x = gridOriginX + (col * spacingX);
					var tile = A2(
						$elm$core$Maybe$withDefault,
						'',
						$elm$core$List$head(
							A2($elm$core$List$drop, index, model.plateauTiles)));
					var isAvailable = A2($elm$core$List$member, index, model.availablePositions);
					var canClick = isAvailable && ((!_Utils_eq(model.currentTile, $elm$core$Maybe$Nothing)) && (!model.showTilePicker));
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class(
								'hex-cell' + (((isAvailable && (!model.showTilePicker)) ? ' available' : '') + ((tile !== '') ? ' filled' : ''))),
								A2(
								$elm$html$Html$Attributes$style,
								'left',
								$elm$core$String$fromFloat(x) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'top',
								$elm$core$String$fromFloat(y) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'width',
								$elm$core$String$fromFloat(hexWidth) + 'px'),
								A2(
								$elm$html$Html$Attributes$style,
								'height',
								$elm$core$String$fromFloat(hexHeight) + 'px'),
								canClick ? $elm$html$Html$Events$onClick(
								$author$project$Main$PlaceRealTile(index)) : $elm$html$Html$Attributes$class('')
							]),
						_List_fromArray(
							[
								function () {
								if (tile !== '') {
									var _v1 = $author$project$TileSvg$parseTileFromPath(tile);
									if (_v1.$ === 'Just') {
										var tileData = _v1.a;
										return A2(
											$elm$html$Html$div,
											_List_fromArray(
												[
													$elm$html$Html$Attributes$class('hex-tile-svg')
												]),
											_List_fromArray(
												[
													$author$project$TileSvg$viewTileSvg(tileData)
												]));
									} else {
										return $elm$html$Html$text('');
									}
								} else {
									return A2($author$project$TileSvg$viewEmptyHexSvg, isAvailable && (!model.showTilePicker), index);
								}
							}()
							]));
				}),
			hexPositions));
};
var $elm$core$List$append = F2(
	function (xs, ys) {
		if (!ys.b) {
			return xs;
		} else {
			return A3($elm$core$List$foldr, $elm$core$List$cons, ys, xs);
		}
	});
var $elm$core$List$concat = function (lists) {
	return A3($elm$core$List$foldr, $elm$core$List$append, _List_Nil, lists);
};
var $elm$core$List$concatMap = F2(
	function (f, list) {
		return $elm$core$List$concat(
			A2($elm$core$List$map, f, list));
	});
var $author$project$Main$SelectRealTile = function (a) {
	return {$: 'SelectRealTile', a: a};
};
var $author$project$Main$viewPickerTile = F2(
	function (usedTiles, tileCode) {
		var tileData = $author$project$TileSvg$parseTileFromPath('image/' + (tileCode + '.png'));
		var isUsed = A2($elm$core$List$member, tileCode, usedTiles);
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class(
					'picker-tile' + (isUsed ? ' used' : '')),
					isUsed ? $elm$html$Html$Attributes$class('') : $elm$html$Html$Events$onClick(
					$author$project$Main$SelectRealTile(tileCode))
				]),
			_List_fromArray(
				[
					function () {
					if (tileData.$ === 'Just') {
						var td = tileData.a;
						return $author$project$TileSvg$viewTileSvg(td);
					} else {
						return $elm$html$Html$text(tileCode);
					}
				}(),
					isUsed ? A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('used-overlay')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text('✓')
						])) : $elm$html$Html$text('')
				]));
	});
var $author$project$Main$viewTilePicker = function (model) {
	var tilesForV1 = function (v1) {
		return A2(
			$elm$core$List$concatMap,
			function (v2) {
				return A2(
					$elm$core$List$map,
					function (v3) {
						return _Utils_ap(
							$elm$core$String$fromInt(v1),
							_Utils_ap(
								$elm$core$String$fromInt(v2),
								$elm$core$String$fromInt(v3)));
					},
					_List_fromArray(
						[3, 4, 8]));
			},
			_List_fromArray(
				[2, 6, 7]));
	};
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('tile-picker glass-container')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$h3,
				_List_Nil,
				_List_fromArray(
					[
						$elm$html$Html$text('🎲 Sélectionnez la tuile tirée')
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('tiles-rows')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('tiles-row')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$span,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row-label')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('1')
									])),
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row-tiles')
									]),
								A2(
									$elm$core$List$map,
									$author$project$Main$viewPickerTile(model.usedTiles),
									tilesForV1(1)))
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('tiles-row')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$span,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row-label')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('5')
									])),
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row-tiles')
									]),
								A2(
									$elm$core$List$map,
									$author$project$Main$viewPickerTile(model.usedTiles),
									tilesForV1(5)))
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('tiles-row')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$span,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row-label')
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('9')
									])),
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('row-tiles')
									]),
								A2(
									$elm$core$List$map,
									$author$project$Main$viewPickerTile(model.usedTiles),
									tilesForV1(9)))
							]))
					]))
			]));
};
var $author$project$Main$viewRealGame = function (model) {
	var playerScore = $author$project$Main$calculateBoardScore(model.plateauTiles);
	var aiScore = $author$project$Main$calculateBoardScore(model.aiPlateauTiles);
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('real-game-container')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('real-game-info glass-container')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$h2,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text(
								'Tour ' + ($elm$core$String$fromInt(model.currentTurnNumber + 1) + '/19'))
							])),
						A2(
						$elm$html$Html$p,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text(
								'Tuiles utilisées: ' + ($elm$core$String$fromInt(
									$elm$core$List$length(model.usedTiles)) + '/27'))
							])),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('reset-button'),
								$elm$html$Html$Events$onClick($author$project$Main$ResetRealGame)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('🔄 Recommencer')
							]))
					])),
				model.showTilePicker ? $author$project$Main$viewTilePicker(model) : A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('current-tile-section glass-container')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$h3,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Tuile sélectionnée')
							])),
						function () {
						var _v0 = model.currentTileImage;
						if (_v0.$ === 'Just') {
							var img = _v0.a;
							var _v1 = $author$project$TileSvg$parseTileFromPath(img);
							if (_v1.$ === 'Just') {
								var tileData = _v1.a;
								return A2(
									$elm$html$Html$div,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('selected-tile-display')
										]),
									_List_fromArray(
										[
											$author$project$TileSvg$viewTileSvg(tileData)
										]));
							} else {
								return $elm$html$Html$text('');
							}
						} else {
							return $elm$html$Html$text('');
						}
					}(),
						A2(
						$elm$html$Html$p,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Cliquez sur une case pour placer la tuile')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('real-game-boards')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('game-board glass-container')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$h3,
								_List_Nil,
								_List_fromArray(
									[
										$elm$html$Html$text(
										'Votre plateau - ' + ($elm$core$String$fromInt(playerScore) + ' pts'))
									])),
								$author$project$Main$viewRealGameBoard(model)
							])),
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('game-board glass-container ai-board')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$h3,
								_List_Nil,
								_List_fromArray(
									[
										$elm$html$Html$text(
										'Plateau IA - ' + ($elm$core$String$fromInt(aiScore) + ' pts'))
									])),
								$author$project$Main$viewAiRealGameBoard(model)
							]))
					])),
				(model.currentTurnNumber >= 19) ? A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('game-over glass-container')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$h2,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Partie terminée!')
							])),
						A2(
						$elm$html$Html$p,
						_List_fromArray(
							[
								A2($elm$html$Html$Attributes$style, 'font-size', '1.2em')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(
								'Votre score: ' + ($elm$core$String$fromInt(playerScore) + ' pts'))
							])),
						A2(
						$elm$html$Html$p,
						_List_fromArray(
							[
								A2($elm$html$Html$Attributes$style, 'font-size', '1.2em')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text(
								'Score IA: ' + ($elm$core$String$fromInt(aiScore) + ' pts'))
							])),
						(_Utils_cmp(playerScore, aiScore) > 0) ? A2(
						$elm$html$Html$p,
						_List_fromArray(
							[
								A2($elm$html$Html$Attributes$style, 'font-size', '1.3em'),
								A2($elm$html$Html$Attributes$style, 'font-weight', 'bold')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Vous avez gagné!')
							])) : ((_Utils_cmp(aiScore, playerScore) > 0) ? A2(
						$elm$html$Html$p,
						_List_fromArray(
							[
								A2($elm$html$Html$Attributes$style, 'font-size', '1.3em'),
								A2($elm$html$Html$Attributes$style, 'font-weight', 'bold')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('L\'IA a gagné!')
							])) : A2(
						$elm$html$Html$p,
						_List_fromArray(
							[
								A2($elm$html$Html$Attributes$style, 'font-size', '1.3em'),
								A2($elm$html$Html$Attributes$style, 'font-weight', 'bold')
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Égalité!')
							]))),
						A2(
						$elm$html$Html$button,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('play-again-button'),
								$elm$html$Html$Events$onClick($author$project$Main$ResetRealGame)
							]),
						_List_fromArray(
							[
								$elm$html$Html$text('Nouvelle partie')
							]))
					])) : $elm$html$Html$text('')
			]));
};
var $author$project$Main$viewGame = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('game-container')
			]),
		_List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('header-section')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('title-with-back')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$button,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('back-button'),
										$elm$html$Html$Events$onClick($author$project$Main$BackToModeSelection)
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('← Retour')
									])),
								A2(
								$elm$html$Html$h1,
								_List_Nil,
								_List_fromArray(
									[
										$elm$html$Html$text(
										function () {
											var _v0 = model.selectedGameMode;
											if (_v0.$ === 'Just') {
												var mode = _v0.a;
												return mode.icon + (' ' + mode.name);
											} else {
												return 'Take It Easy';
											}
										}())
									]))
							]))
					])),
				(model.error !== '') ? A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('error-message')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text(model.error)
					])) : $elm$html$Html$text(''),
				(model.statusMessage !== '') ? A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('status-message')
					]),
				_List_fromArray(
					[
						$elm$html$Html$text(model.statusMessage)
					])) : $elm$html$Html$text(''),
				function () {
				if (model.isRealGameMode) {
					return $author$project$Main$viewRealGame(model);
				} else {
					var _v1 = model.session;
					if (_v1.$ === 'Nothing') {
						return $author$project$Main$viewConnectionInterface(model);
					} else {
						var session = _v1.a;
						return A2($author$project$Main$viewGameSession, model, session);
					}
				}
			}()
			]));
};
var $author$project$Main$StartGame = {$: 'StartGame'};
var $author$project$Main$SelectGameMode = function (a) {
	return {$: 'SelectGameMode', a: a};
};
var $author$project$Main$viewModeCard = F2(
	function (selectedMode, mode) {
		var isSelected = _Utils_eq(
			A2(
				$elm$core$Maybe$map,
				function ($) {
					return $.id;
				},
				selectedMode),
			$elm$core$Maybe$Just(mode.id));
		return A2(
			$elm$html$Html$div,
			_List_fromArray(
				[
					$elm$html$Html$Attributes$class(
					'mode-card' + (isSelected ? ' selected' : '')),
					$elm$html$Html$Events$onClick(
					$author$project$Main$SelectGameMode(mode))
				]),
			_List_fromArray(
				[
					function () {
					var _v0 = mode.difficulty;
					if (_v0.$ === 'Just') {
						var diff = _v0.a;
						return A2(
							$elm$html$Html$span,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('difficulty-badge difficulty-' + diff)
								]),
							_List_fromArray(
								[
									$elm$html$Html$text(diff)
								]));
					} else {
						return $elm$html$Html$text('');
					}
				}(),
					A2(
					$elm$html$Html$div,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('mode-icon')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(mode.icon)
						])),
					A2(
					$elm$html$Html$h3,
					_List_Nil,
					_List_fromArray(
						[
							$elm$html$Html$text(mode.name)
						])),
					A2(
					$elm$html$Html$p,
					_List_fromArray(
						[
							$elm$html$Html$Attributes$class('mode-description')
						]),
					_List_fromArray(
						[
							$elm$html$Html$text(mode.description)
						]))
				]));
	});
var $author$project$Main$GoToLogin = {$: 'GoToLogin'};
var $author$project$Main$Logout = {$: 'Logout'};
var $elm$html$Html$strong = _VirtualDom_node('strong');
var $author$project$Main$viewUserHeader = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('user-header')
			]),
		_List_fromArray(
			[
				function () {
				if (model.isAuthenticated) {
					var _v0 = model.user;
					if (_v0.$ === 'Just') {
						var user = _v0.a;
						return A2(
							$elm$html$Html$div,
							_List_fromArray(
								[
									$elm$html$Html$Attributes$class('user-info')
								]),
							_List_fromArray(
								[
									A2(
									$elm$html$Html$span,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('user-name')
										]),
									_List_fromArray(
										[
											$elm$html$Html$text('Connecté: '),
											A2(
											$elm$html$Html$strong,
											_List_Nil,
											_List_fromArray(
												[
													$elm$html$Html$text(user.username)
												]))
										])),
									A2(
									$elm$html$Html$button,
									_List_fromArray(
										[
											$elm$html$Html$Attributes$class('logout-button'),
											$elm$html$Html$Events$onClick($author$project$Main$Logout)
										]),
									_List_fromArray(
										[
											$elm$html$Html$text('Déconnexion')
										]))
								]));
					} else {
						return $elm$html$Html$text('');
					}
				} else {
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('guest-info')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$span,
								_List_Nil,
								_List_fromArray(
									[
										$elm$html$Html$text('Mode invité')
									])),
								A2(
								$elm$html$Html$button,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('login-link'),
										$elm$html$Html$Events$onClick($author$project$Main$GoToLogin)
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('Se connecter')
									]))
							]));
				}
			}()
			]));
};
var $author$project$Main$viewModeSelection = function (model) {
	return A2(
		$elm$html$Html$div,
		_List_fromArray(
			[
				$elm$html$Html$Attributes$class('game-mode-selector')
			]),
		_List_fromArray(
			[
				$author$project$Main$viewUserHeader(model),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('header')
					]),
				_List_fromArray(
					[
						A2(
						$elm$html$Html$h1,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Take It Easy')
							])),
						A2(
						$elm$html$Html$p,
						_List_Nil,
						_List_fromArray(
							[
								$elm$html$Html$text('Choisissez votre mode de jeu')
							]))
					])),
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('modes-grid')
					]),
				A2(
					$elm$core$List$map,
					$author$project$Main$viewModeCard(model.selectedGameMode),
					model.gameModes)),
				function () {
				var _v0 = model.selectedGameMode;
				if (_v0.$ === 'Just') {
					var mode = _v0.a;
					return A2(
						$elm$html$Html$div,
						_List_fromArray(
							[
								$elm$html$Html$Attributes$class('action-panel')
							]),
						_List_fromArray(
							[
								A2(
								$elm$html$Html$div,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('selected-mode-info')
									]),
								_List_fromArray(
									[
										A2(
										$elm$html$Html$h3,
										_List_Nil,
										_List_fromArray(
											[
												$elm$html$Html$text(mode.icon + (' ' + mode.name))
											])),
										A2(
										$elm$html$Html$p,
										_List_Nil,
										_List_fromArray(
											[
												$elm$html$Html$text(mode.description)
											]))
									])),
								A2(
								$elm$html$Html$button,
								_List_fromArray(
									[
										$elm$html$Html$Attributes$class('start-button'),
										$elm$html$Html$Events$onClick($author$project$Main$StartGame)
									]),
								_List_fromArray(
									[
										$elm$html$Html$text('Commencer'),
										A2(
										$elm$html$Html$span,
										_List_fromArray(
											[
												$elm$html$Html$Attributes$class('start-icon')
											]),
										_List_fromArray(
											[
												$elm$html$Html$text(' →')
											]))
									]))
							]));
				} else {
					return $elm$html$Html$text('');
				}
			}()
			]));
};
var $author$project$Main$view = function (model) {
	return {
		body: _List_fromArray(
			[
				A2(
				$elm$html$Html$div,
				_List_fromArray(
					[
						$elm$html$Html$Attributes$class('app-container')
					]),
				_List_fromArray(
					[
						function () {
						var _v0 = model.currentView;
						switch (_v0.$) {
							case 'LoginView':
								return $author$project$Main$viewAuth(model);
							case 'ModeSelectionView':
								return $author$project$Main$viewModeSelection(model);
							default:
								return $author$project$Main$viewGame(model);
						}
					}()
					]))
			]),
		title: 'Take It Easy - Elm'
	};
};
var $author$project$Main$main = $elm$browser$Browser$application(
	{init: $author$project$Main$init, onUrlChange: $author$project$Main$UrlChanged, onUrlRequest: $author$project$Main$UrlRequested, subscriptions: $author$project$Main$subscriptions, update: $author$project$Main$update, view: $author$project$Main$view});
_Platform_export({'Main':{'init':$author$project$Main$main(
	$elm$json$Json$Decode$succeed(_Utils_Tuple0))(0)}});}(this));