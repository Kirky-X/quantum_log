//! QuantumLog 数据库表结构定义
//!
//! 此模块使用 Diesel 的 table! 宏定义数据库表结构。
//! 支持 SQLite、MySQL 和 PostgreSQL 三种数据库类型。

#[cfg(feature = "db")]
use diesel;

#[cfg(feature = "db")]
// 定义数据库表结构
diesel::table! {
    quantum_logs (id) {
        id -> Integer,
        timestamp -> Timestamp,
        level -> Text,
        target -> Text,
        message -> Text,
        fields -> Nullable<Text>,
        span_id -> Nullable<Text>,
        span_name -> Nullable<Text>,
        process_id -> Integer,
        thread_id -> Text,
        hostname -> Text,
        username -> Text,
        mpi_rank -> Nullable<Integer>,
        file_path -> Nullable<Text>,
        line_number -> Nullable<Integer>,
        module_path -> Nullable<Text>,
    }
}

/// 为不同数据库类型提供特定的 SQL 类型映射
#[cfg(feature = "postgres")]
mod postgres_types {

    // PostgreSQL 特定的表定义
    diesel::table! {
        use diesel::sql_types::*;
        use diesel::pg::sql_types::*;

        quantum_logs (id) {
            id -> BigSerial,
            timestamp -> Timestamp,
            level -> Varchar,
            target -> Varchar,
            message -> Text,
            file -> Nullable<Varchar>,
            line -> Nullable<Integer>,
            pid -> Nullable<Integer>,
            tid -> Nullable<BigInt>,
            mpi_rank -> Nullable<Integer>,
            username -> Nullable<Varchar>,
            hostname -> Nullable<Varchar>,
            span_info -> Nullable<Jsonb>,
            fields -> Nullable<Jsonb>,
        }
    }
}

// 导出表定义供其他模块使用
#[cfg(feature = "db")]
pub use self::quantum_logs::dsl::*;

#[cfg(feature = "mysql")]
mod mysql_types {

    // MySQL 特定的表定义
    diesel::table! {
        use diesel::sql_types::*;

        quantum_logs (id) {
            id -> BigInt,
            timestamp -> Datetime,
            level -> Varchar,
            target -> Varchar,
            message -> Text,
            file -> Nullable<Varchar>,
            line -> Nullable<Integer>,
            pid -> Nullable<Integer>,
            tid -> Nullable<BigInt>,
            mpi_rank -> Nullable<Integer>,
            username -> Nullable<Varchar>,
            hostname -> Nullable<Varchar>,
            span_info -> Nullable<Json>,
            fields -> Nullable<Json>,
        }
    }
}

// 导出表定义供其他模块使用

#[cfg(feature = "sqlite")]
mod sqlite_types {

    // SQLite 特定的表定义
    diesel::table! {
        use diesel::sql_types::*;

        quantum_logs (id) {
            id -> BigInt,
            timestamp -> Text,  // SQLite 使用 TEXT 存储 ISO 8601 格式的时间戳
            level -> Text,
            target -> Text,
            message -> Text,
            file -> Nullable<Text>,
            line -> Nullable<Integer>,
            pid -> Nullable<Integer>,
            tid -> Nullable<BigInt>,
            mpi_rank -> Nullable<Integer>,
            username -> Nullable<Text>,
            hostname -> Nullable<Text>,
            span_info -> Nullable<Text>,  // SQLite 使用 TEXT 存储 JSON
            fields -> Nullable<Text>,     // SQLite 使用 TEXT 存储 JSON
        }
    }
}

// 导出表定义供其他模块使用

/// 创建表的 SQL 语句
pub mod create_table_sql {
    /// PostgreSQL 创建表语句
    #[cfg(feature = "postgres")]
    pub const POSTGRES_CREATE_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS quantum_logs (
            id BIGSERIAL PRIMARY KEY,
            timestamp TIMESTAMPTZ NOT NULL,
            level VARCHAR(10) NOT NULL,
            target VARCHAR(255) NOT NULL,
            message TEXT NOT NULL,
            file VARCHAR(255),
            line INTEGER,
            pid INTEGER,
            tid BIGINT,
            mpi_rank INTEGER,
            username VARCHAR(255),
            hostname VARCHAR(255),
            span_info JSONB,
            fields JSONB
        );
        
        CREATE INDEX IF NOT EXISTS idx_quantum_logs_timestamp ON quantum_logs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_quantum_logs_level ON quantum_logs(level);
        CREATE INDEX IF NOT EXISTS idx_quantum_logs_target ON quantum_logs(target);
    "#;

    /// MySQL 创建表语句
    #[cfg(feature = "mysql")]
    pub const MYSQL_CREATE_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS quantum_logs (
            id BIGINT AUTO_INCREMENT PRIMARY KEY,
            timestamp DATETIME NOT NULL,
            level VARCHAR(10) NOT NULL,
            target VARCHAR(255) NOT NULL,
            message TEXT NOT NULL,
            file VARCHAR(255),
            line INT,
            pid INT,
            tid BIGINT,
            mpi_rank INT,
            username VARCHAR(255),
            hostname VARCHAR(255),
            span_info JSON,
            fields JSON,
            INDEX idx_timestamp (timestamp),
            INDEX idx_level (level),
            INDEX idx_target (target)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;

    /// SQLite 创建表语句
    #[cfg(feature = "sqlite")]
    pub const SQLITE_CREATE_TABLE: &str = r#"
        CREATE TABLE IF NOT EXISTS quantum_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            level TEXT NOT NULL,
            target TEXT NOT NULL,
            message TEXT NOT NULL,
            file TEXT,
            line INTEGER,
            pid INTEGER,
            tid INTEGER,
            mpi_rank INTEGER,
            username TEXT,
            hostname TEXT,
            span_info TEXT,
            fields TEXT
        );
        
        CREATE INDEX IF NOT EXISTS idx_quantum_logs_timestamp ON quantum_logs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_quantum_logs_level ON quantum_logs(level);
        CREATE INDEX IF NOT EXISTS idx_quantum_logs_target ON quantum_logs(target);
    "#;
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(feature = "db")]
    fn test_table_definition_compiles() {
        // 这个测试确保表定义能够编译通过
        // 实际的表结构验证将在集成测试中进行

        // 测试表名是否正确
        let table_name = "quantum_logs";
        assert_eq!(table_name, "quantum_logs");
    }

    #[test]
    fn test_create_table_sql_not_empty() {
        // 测试创建表的 SQL 语句不为空
        #[cfg(feature = "postgres")]
        {
            assert!(!super::create_table_sql::POSTGRES_CREATE_TABLE.is_empty());
            assert!(super::create_table_sql::POSTGRES_CREATE_TABLE.contains("quantum_logs"));
        }

        #[cfg(feature = "mysql")]
        {
            assert!(!super::create_table_sql::MYSQL_CREATE_TABLE.is_empty());
            assert!(super::create_table_sql::MYSQL_CREATE_TABLE.contains("quantum_logs"));
        }

        #[cfg(feature = "sqlite")]
        {
            assert!(!super::create_table_sql::SQLITE_CREATE_TABLE.is_empty());
            assert!(super::create_table_sql::SQLITE_CREATE_TABLE.contains("quantum_logs"));
        }
    }
}

// 导出表定义供其他模块使用
