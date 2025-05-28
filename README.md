# 🧱 Rustite: SQLite ORM-style Binding for Node.js

Rustite is a native Node.js module powered by [Rust](https://www.rust-lang.org/) and [NAPI-RS](https://napi.rs/), providing a lightweight ORM-style interface to SQLite databases.

---

## 📦 Installation

```bash
npm install rustqlite
```

## 🚀 Quick Start

1. Initialize the Database
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

2. Insert a Record
```js
const users = db.table('users');

const id = users.insert({ name: 'Alice', age: 30 });
console.log('Inserted user with ID:', id);
```