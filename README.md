# 🧱 Rustite: SQLite ORM-style Binding for Node.js

Rustite is a native Node.js module powered by [Rust](https://www.rust-lang.org/) and [NAPI-RS](https://napi.rs/), providing a lightweight ORM-style interface to SQLite databases.

---

## 📦 Installation

```bash
npm install rustqlite
```

## 🚀 Quick Start

### 1. Initialize the Database
```js
const { Database } = require('rustqlite');

const db = new Database('mydb.sqlite');

db.execute(`
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

const id = users.insert({ name: 'Alice', age: 30 });
console.log('Inserted user with ID:', id);
```

### 3. Find a Record by ID
```js
const user = users.find(1);
console.log('User:', user?.data);
```

### 4. Query with Conditions
```js
const adults = users.where('age', '>=', '18');

console.log('Adults:', adults.data);

const first = adults.first();
const last = adults.last();
```

### 5. Update a Record
```js
const user = users.find(1);
if (user) {
  user.update({ age: 31 });
}
```

### 6. Delete a Record
```js
const user = users.find(1);
if (user) {
  user.destroy(); // or user.delete();
}
```

### 7. Custom SQL Queries
```js
const result = db.queryAll('SELECT name FROM users WHERE age > 20');
console.log(result);
```

## 🧠 API Overview

### `Database`

* `new(path: string)`
  * Opens or creates a SQLite database at the given file path.

* `table(name: string): Table`
  * Returns a `Table` instance bound to a specific table name.

* `execute(sql: string): void`
  * Executes raw SQL (e.g., CREATE TABLE, DROP TABLE, etc).

* `query_all(sql: string): string (JSON)`
  * Executes a SELECT query and returns the results as a JSON string.

---

### `Table`

* `find(id: number): Record | null`
  * Retrieves a single record by its primary key `id`.

* `insert(obj: object): number`
  * Inserts a new row with the given object fields. Returns the new row ID.

* `where(column: string, op: string, value: string): RecordList`
  * Performs a filtered query (e.g., `where("age", ">=", "21")`).

* `first(): Record | null`
  * Returns the first record in ascending `id` order.

* `last(): Record | null`
  * Returns the last record in descending `id` order.

---

### `Record`

* `.data: object`
  * JSON representation of the row.

* `update(obj: object): Record`
  * Updates the row with new values from `obj`.

* `destroy(): void`
  * Permanently deletes the row from the table.

* `delete(): void`
  * Alias for `destroy()`.

---

### `RecordList`

* `.data: object[]`
  * Array of JSON records contained in the result set.

* `first(): Record | null`
  * First record in the list.

* `last(): Record | null`
  * Last record in the list.

---

# 🧪 Full Example

```js
const db = new Database('app.sqlite');
db.execute(`CREATE TABLE IF NOT EXISTS posts (id INTEGER PRIMARY KEY, title TEXT, content TEXT)`);

const posts = db.table('posts');

const postId = posts.insert({ title: 'Hello', content: 'World' });

const post = posts.find(postId);
post.update({ content: 'Updated content' });

const allPosts = posts.where('id', '>=', '1');
console.log(allPosts.data);

post.destroy();
```