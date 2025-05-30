# 🧱 Rustite: SQLite ORM-style Binding for Node.js

Rustite is a native Node.js module powered by [Rust](https://www.rust-lang.org/) and [NAPI-RS](https://napi.rs/), providing a lightweight ORM-style interface to SQLite databases.

---

## 📦 Installation

```bash
npm install rustite
```

## 🚀 Quick Start

### 1. Initialize the Database

```js
const { Database } = require('rustite');

const db = new Database('mydb.sqlite');

await db.execute(`
  CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    age INTEGER
  )
`);
```

### 2. Insert a Record

```js
const users = db.table('users');

await users.insert({ name: 'Alice', age: 30 });
```

### 3. Find a Record by ID

```js
const user = await users.find(1);
console.log('User:', user);
```

### 4. Query with Conditions

```js
const adults = users.where('age', '>=', 18);

const result = await adults.get();
console.log('Adults:', result);

const first = await adults.first();
const last = await adults.last();
```

### 5. Update a Record

```js
await users.update(1, { age: 31 });

// Or

const user = await users.find(1);
if (user) {
  await user.update({ age: 31 });
}
```

### 6. Delete a Record

```js
await users.destroy(1);

// Or

const user = await users.find(1);
if (user) {
  await user.destroy();
}
```

### 7. Chained Queries and Nested Conditions

```js
const result = await users
  .where('age', '>=', 18)
  .where('name', 'LIKE', '%A%')
  .order_by('id', 'DESC')
  .get();

console.log(result);
```

### 8. Custom SQL Queries

```js
const rows = await db.query('SELECT name FROM users WHERE age > 20');
console.log(rows);
```

## 🧠 API Overview

### `Database`

* `new(path: string)`

  * Opens or creates a SQLite database at the given file path.

* `table(name: string): Table`

  * Returns a `Table` instance bound to a specific table name.

* `execute(sql: string): Promise<void>`

  * Executes raw SQL (e.g., CREATE TABLE, DROP TABLE, etc).

* `query(sql: string): Promise<object[]>`

  * Executes a SELECT query and returns the results as parsed objects.

---

### `Table`

* `find(id: string | number): Promise<Record | null>`

  * Retrieves a single record by its primary key `id`.

* `insert(obj: object): Promise<number>`

  * Inserts a new row with the given object fields. Returns the new row ID.

* `where(column: string, op: string, value: any): Table`

  * Adds a filter condition (e.g., `where("age", ">=", 21)`). Supports `=`, `!=`, `<`, `>`, `LIKE`, `IN`, `IS NULL`, and more.

* `order_by(column: string, direction?: 'ASC' | 'DESC'): Table`

  * Adds ordering to the current query.

* `get(): Promise<object[]>`

  * Executes the built query and returns the list of matching records.

* `first(): Promise<object | null>`

  * Returns the first matching record ordered by ascending `id`.

* `last(): Promise<object | null>`

  * Returns the last matching record ordered by descending `id`.

* `update(id: string | number, object: object): Promise<void>`

  * Updates the record with the given ID.

* `destroy(id: string | number): Promise<void>`

  * Deletes the record with the given ID.

---

### `Record` (Instance returned by `.find()`)

* `update(object: object): Promise<void>`

  * Updates the current record in the database.

* `destroy(): Promise<void>`

  * Deletes the current record from the database.

---

# 🥪 Full Example

```js
const { Database } = require('rustite');

async function main() {
  const db = new Database('app.sqlite');
  await db.execute(`CREATE TABLE IF NOT EXISTS posts (id INTEGER PRIMARY KEY, title TEXT, content TEXT)`);

  const posts = db.table('posts');

  const postId = await posts.insert({ title: 'Hello', content: 'World' });

  const post = await posts.find(postId);
  if (post) await post.update({ content: 'Updated content' });

  const allPosts = await posts.where('id', '>=', 1).get();
  console.log(allPosts);

  if (post) await post.destroy();
}

main();
```
