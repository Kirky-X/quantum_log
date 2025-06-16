//!
//! 数据库 Sink 实现
//!
//! 此模块提供了将日志写入各种数据库的功能，支持 SQLite、MySQL 和 PostgreSQL。
//! 使用连接池和批量插入来优化性能。

use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::config::{DatabaseSinkConfig, DatabaseType};
use crate::core::event::QuantumLogEvent;
use crate::error::QuantumLogError;
use crate::sinks::database::models::{NewQuantumLogEntry, LogBatch};

type Result<T> = std::result::Result<T, QuantumLogError>;

#[cfg(feature = "database")]
use diesel::prelude::*;
#[cfg(feature = "database")]
use diesel::r2d2::{ConnectionManager, Pool};

/// 数据库连接池类型别名
#[cfg(all(feature = "database", feature = "sqlite"))]
type SqlitePool = Pool<ConnectionManager<diesel::sqlite::SqliteConnection>>;

#[cfg(all(feature = "database", feature = "mysql"))]
type MysqlPool = Pool<ConnectionManager<diesel::mysql::MysqlConnection>>;

#[cfg(all(feature = "database", feature = "postgres"))]
type PostgresPool = Pool<ConnectionManager<diesel::pg::PgConnection>>;

/// 数据库连接池枚举
#[cfg(feature = "database")]
#[derive(Clone)]
pub enum DatabasePool {
    #[cfg(feature = "sqlite")]
    Sqlite(SqlitePool),
    #[cfg(feature = "mysql")]
    Mysql(MysqlPool),
    #[cfg(feature = "postgres")]
    Postgres(PostgresPool),
}

/// 数据库 Sink 结构体
///
/// 负责将日志事件批量写入数据库，支持多种数据库类型。
#[derive(Clone)]
pub struct DatabaseSink {
    /// 数据库连接池
    #[cfg(feature = "database")]
    pool: DatabasePool,
    /// 配置信息
    config: DatabaseSinkConfig,
    /// 表名（包含 schema 前缀）
    full_table_name: String,
}

impl std::fmt::Debug for DatabaseSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseSink")
            .field("config", &self.config)
            .field("full_table_name", &self.full_table_name)
            .field("pool", &"<DatabasePool>")
            .finish()
    }
}

impl DatabaseSink {
    /// 创建新的数据库 Sink 实例
    ///
    /// # 参数
    /// * `config` - 数据库配置
    ///
    /// # 返回
    /// 返回配置好的 DatabaseSink 实例或错误
    #[cfg(feature = "database")]
    pub async fn new(config: DatabaseSinkConfig) -> Result<Self> {
        let pool = Self::create_connection_pool(&config).await?;

        let full_table_name = if let Some(ref schema) = config.schema_name {
            format!("{}.{}", schema, config.table_name)
        } else {
            config.table_name.clone()
        };

        let sink = Self {
            pool,
            config: config.clone(),
            full_table_name,
        };

        // 如果启用了自动创建表，则创建表
        if config.auto_create_table {
            sink.create_table_if_not_exists().await?;
        }

        Ok(sink)
    }

    /// 创建数据库连接池
    #[cfg(feature = "database")]
    async fn create_connection_pool(config: &DatabaseSinkConfig) -> Result<DatabasePool> {
        use diesel::r2d2::Pool;

        let timeout_duration = Duration::from_millis(config.connection_timeout_ms);

        match config.db_type {
            #[cfg(feature = "sqlite")]
            DatabaseType::Sqlite => {
                let manager = ConnectionManager::<diesel::sqlite::SqliteConnection>::new(
                    &config.connection_string,
                );
                let pool = Pool::builder()
                    .max_size(config.connection_pool_size)
                    .connection_timeout(timeout_duration)
                    .build(manager)
                    .map_err(|e| {
                        QuantumLogError::DatabaseError(format!("SQLite 连接池创建失败: {}", e))
                    })?;
                Ok(DatabasePool::Sqlite(pool))
            }
            #[cfg(feature = "mysql")]
            DatabaseType::Mysql => {
                let manager = ConnectionManager::<diesel::mysql::MysqlConnection>::new(
                    &config.connection_string,
                );
                let pool = Pool::builder()
                    .max_size(config.connection_pool_size)
                    .connection_timeout(timeout_duration)
                    .build(manager)
                    .map_err(|e| {
                        QuantumLogError::DatabaseError(format!("MySQL 连接池创建失败: {}", e))
                    })?;
                Ok(DatabasePool::Mysql(pool))
            }
            #[cfg(feature = "postgres")]
            DatabaseType::Postgresql => {
                let manager =
                    ConnectionManager::<diesel::pg::PgConnection>::new(&config.connection_string);
                let pool = Pool::builder()
                    .max_size(config.connection_pool_size)
                    .connection_timeout(timeout_duration)
                    .build(manager)
                    .map_err(|e| {
                        QuantumLogError::DatabaseError(format!("PostgreSQL 连接池创建失败: {}", e))
                    })?;
                Ok(DatabasePool::Postgres(pool))
            }
            #[cfg(not(feature = "sqlite"))]
            DatabaseType::Sqlite => Err(QuantumLogError::DatabaseError(
                "SQLite support not enabled".to_string(),
            )),
            #[cfg(not(feature = "mysql"))]
            DatabaseType::Mysql => Err(QuantumLogError::DatabaseError(
                "MySQL support not enabled".to_string(),
            )),
            #[cfg(not(feature = "postgres"))]
            DatabaseType::Postgresql => Err(QuantumLogError::DatabaseError(
                "PostgreSQL support not enabled".to_string(),
            )),
        }
    }

    /// 发送事件到数据库
    #[cfg(feature = "database")]
    pub async fn send_event(
        &self,
        event: QuantumLogEvent,
        _strategy: &crate::config::BackpressureStrategy,
    ) -> Result<()> {
        // 将事件转换为数据库条目
        let entry = self.convert_event_to_entry(&event).await?;

        // 直接插入单个条目
        let pool = self.pool.clone();
        let entries = vec![entry];

        tokio::task::spawn_blocking(move || Self::insert_batch_blocking(pool, entries))
            .await
            .map_err(|e| QuantumLogError::DatabaseError(format!("数据库插入任务执行失败: {}", e)))?
    }

    /// 关闭数据库 Sink
    #[cfg(feature = "database")]
    pub async fn shutdown(self) -> Result<()> {
        // 数据库 Sink 使用 spawn_task 模式，关闭由任务内部处理
        // 这里只需要返回成功即可
        Ok(())
    }

    /// 创建表（如果不存在）
    #[cfg(feature = "database")]
    async fn create_table_if_not_exists(&self) -> Result<()> {
        use crate::sinks::database::schema::create_table_sql;

        let sql = match self.config.db_type {
            #[cfg(feature = "sqlite")]
            DatabaseType::Sqlite => create_table_sql::SQLITE_CREATE_TABLE,
            #[cfg(feature = "mysql")]
            DatabaseType::Mysql => create_table_sql::MYSQL_CREATE_TABLE,
            #[cfg(feature = "postgres")]
            DatabaseType::Postgresql => create_table_sql::POSTGRES_CREATE_TABLE,
            #[cfg(not(feature = "sqlite"))]
            DatabaseType::Sqlite => {
                return Err(QuantumLogError::ConfigError(
                    "SQLite support not enabled".to_string(),
                ))
            }
            #[cfg(not(feature = "mysql"))]
            DatabaseType::Mysql => {
                return Err(QuantumLogError::ConfigError(
                    "MySQL support not enabled".to_string(),
                ))
            }
            #[cfg(not(feature = "postgres"))]
            DatabaseType::Postgresql => {
                return Err(QuantumLogError::ConfigError(
                    "PostgreSQL support not enabled".to_string(),
                ))
            }
        };

        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            match pool {
                #[cfg(feature = "sqlite")]
                DatabasePool::Sqlite(pool) => {
                    let mut conn = pool.get().map_err(|e| {
                        QuantumLogError::DatabaseError(format!("获取 SQLite 连接失败: {}", e))
                    })?;
                    diesel::sql_query(sql).execute(&mut conn).map_err(|e| {
                        QuantumLogError::DatabaseError(format!("SQLite 表创建失败: {}", e))
                    })?;
                }
                #[cfg(feature = "mysql")]
                DatabasePool::Mysql(pool) => {
                    let mut conn = pool.get().map_err(|e| {
                        QuantumLogError::DatabaseError(format!("获取 MySQL 连接失败: {}", e))
                    })?;
                    diesel::sql_query(sql).execute(&mut conn).map_err(|e| {
                        QuantumLogError::DatabaseError(format!("MySQL 表创建失败: {}", e))
                    })?;
                }
                #[cfg(feature = "postgres")]
                DatabasePool::Postgres(pool) => {
                    let mut conn = pool.get().map_err(|e| {
                        QuantumLogError::DatabaseError(format!("获取 PostgreSQL 连接失败: {}", e))
                    })?;
                    diesel::sql_query(sql).execute(&mut conn).map_err(|e| {
                        QuantumLogError::DatabaseError(format!("PostgreSQL 表创建失败: {}", e))
                    })?;
                }
            }
            Ok::<(), QuantumLogError>(())
        })
        .await
        .map_err(|e| QuantumLogError::DatabaseError(format!("表创建任务执行失败: {}", e)))?
    }

    /// 启动数据库 Sink 任务
    ///
    /// # 参数
    /// * `mut receiver` - 接收日志事件的通道
    /// * `shutdown_signal` - 停机信号接收器
    ///
    /// # 返回
    /// 返回任务句柄
    #[cfg(feature = "database")]
    pub fn spawn_task(
        self,
        mut receiver: mpsc::Receiver<QuantumLogEvent>,
        mut shutdown_signal: tokio::sync::broadcast::Receiver<()>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async move {
            let mut batch = LogBatch::new();
            let mut flush_interval = interval(Duration::from_secs(1)); // 每秒检查一次是否需要刷新

            info!(
                "数据库 Sink 任务已启动，数据库类型: {:?}",
                self.config.db_type
            );

            loop {
                tokio::select! {
                    // 接收新的日志事件
                    event = receiver.recv() => {
                        match event {
                            Some(log_event) => {
                                if let Ok(entry) = self.convert_event_to_entry(&log_event).await {
                                    batch.add_entry(entry);

                                    // 检查是否需要刷新批次
                                    if batch.is_full(self.config.batch_size) {
                                        if let Err(e) = self.flush_batch(&mut batch).await {
                                            error!("批量写入数据库失败: {}", e);
                                            if let Some(diagnostics) = crate::diagnostics::get_diagnostics_instance() {
                                                diagnostics.increment_sink_errors();
                                            }
                                        }
                                    }
                                } else {
                                    warn!("转换日志事件为数据库条目失败");
                                }
                            },
                            None => {
                                debug!("日志事件通道已关闭");
                                break;
                            }
                        }
                    },

                    // 定期刷新检查
                    _ = flush_interval.tick() => {
                        if !batch.is_empty() && (batch.is_expired(5) || batch.is_full(self.config.batch_size)) {
                            if let Err(e) = self.flush_batch(&mut batch).await {
                                error!("定期刷新数据库批次失败: {}", e);
                                if let Some(diagnostics) = crate::diagnostics::get_diagnostics_instance() {
                                    diagnostics.increment_sink_errors();
                                }
                            }
                        }
                    },

                    // 接收停机信号
                    _ = shutdown_signal.recv() => {
                        info!("收到停机信号，正在刷新剩余的数据库批次");
                        if !batch.is_empty() {
                            if let Err(e) = self.flush_batch(&mut batch).await {
                                error!("停机时刷新数据库批次失败: {}", e);
                            }
                        }
                        break;
                    }
                }
            }

            info!("数据库 Sink 任务已停止");
            Ok(())
        })
    }

    /// 将 QuantumLogEvent 转换为 NewQuantumLogEntry
    #[cfg(feature = "database")]
    async fn convert_event_to_entry(&self, event: &QuantumLogEvent) -> Result<NewQuantumLogEntry> {
        

        let mut entry = NewQuantumLogEntry::new(
            event.timestamp.naive_utc(),
            event.level.to_string(),
            event.target.clone(),
            event.message.clone(),
            event.context.pid.try_into().unwrap_or(0),
            event.context.tid.to_string(),
            event.context.hostname.clone().unwrap_or_default(),
            event.context.username.clone().unwrap_or_default(),
        );

        // 设置可选字段
        if let Some(ref file_path) = event.file {
            entry = entry.with_file_info(Some(file_path.clone()), event.line.map(|l| l as i32));
        }

        if let Some(ref module_path) = event.module_path {
            entry = entry.with_module_path(Some(module_path.clone()));
        }

        if let Some(mpi_rank) = event.context.mpi_rank {
            entry = entry.with_mpi_rank(Some(mpi_rank));
        }

        // Note: span_id and span_name fields don't exist in QuantumLogEvent
        // This code is commented out until the event structure is updated
        // if let Some(ref span_id) = event.span_id {
        //     entry = entry.with_span_info(Some(span_id.clone()), event.span_name.clone());
        // }

        // 序列化额外字段
        if !event.fields.is_empty() {
            let fields_json = serde_json::to_string(&event.fields)
                .map_err(|e| QuantumLogError::SerializationError { source: e })?;
            entry = entry.with_fields(Some(fields_json));
        }

        Ok(entry)
    }

    /// 刷新批次到数据库
    #[cfg(feature = "database")]
    async fn flush_batch(&self, batch: &mut LogBatch) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let entries = batch.entries.clone();
        let pool = self.pool.clone();
        let batch_size = entries.len();

        debug!("正在刷新 {} 条日志到数据库", batch_size);

        // 使用 spawn_blocking 在阻塞线程池中执行数据库操作
        let result =
            tokio::task::spawn_blocking(move || Self::insert_batch_blocking(pool, entries)).await;

        match result {
            Ok(Ok(())) => {
                debug!("成功写入 {} 条日志到数据库", batch_size);
                if let Some(diagnostics) = crate::diagnostics::get_diagnostics_instance() {
                    diagnostics.add_events_processed(batch_size as u64);
                }
                batch.clear();
                Ok(())
            }
            Ok(Err(e)) => {
                error!("数据库批量插入失败: {}", e);
                Err(e)
            }
            Err(e) => {
                let error_msg = format!("数据库任务执行失败: {}", e);
                error!("{}", error_msg);
                Err(QuantumLogError::DatabaseError(error_msg))
            }
        }
    }

    /// 在阻塞线程中执行批量插入
    #[cfg(feature = "database")]
    fn insert_batch_blocking(pool: DatabasePool, entries: Vec<NewQuantumLogEntry>) -> Result<()> {
        use crate::sinks::database::schema::quantum_logs;

        match pool {
            #[cfg(feature = "sqlite")]
            DatabasePool::Sqlite(pool) => {
                let mut conn = pool.get().map_err(|e| {
                    QuantumLogError::DatabaseError(format!("获取 SQLite 连接失败: {}", e))
                })?;

                diesel::insert_into(quantum_logs::table)
                    .values(&entries)
                    .execute(&mut conn)
                    .map_err(|e| {
                        QuantumLogError::DatabaseError(format!("SQLite 批量插入失败: {}", e))
                    })?;
            }
            #[cfg(feature = "mysql")]
            DatabasePool::Mysql(pool) => {
                let mut conn = pool.get().map_err(|e| {
                    QuantumLogError::DatabaseError(format!("获取 MySQL 连接失败: {}", e))
                })?;

                diesel::insert_into(quantum_logs::table)
                    .values(&entries)
                    .execute(&mut conn)
                    .map_err(|e| {
                        QuantumLogError::DatabaseError(format!("MySQL 批量插入失败: {}", e))
                    })?;
            }
            #[cfg(feature = "postgres")]
            DatabasePool::Postgres(pool) => {
                let mut conn = pool.get().map_err(|e| {
                    QuantumLogError::DatabaseError(format!("获取 PostgreSQL 连接失败: {}", e))
                })?;

                diesel::insert_into(quantum_logs::table)
                    .values(&entries)
                    .execute(&mut conn)
                    .map_err(|e| {
                        QuantumLogError::DatabaseError(format!("PostgreSQL 批量插入失败: {}", e))
                    })?;
            }
        }

        Ok(())
    }

    /// 测试数据库连接
    #[cfg(feature = "database")]
    pub async fn test_connection(&self) -> Result<()> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            match pool {
                #[cfg(feature = "sqlite")]
                DatabasePool::Sqlite(pool) => {
                    let _conn = pool.get().map_err(|e| {
                        QuantumLogError::DatabaseError(format!("SQLite 连接测试失败: {}", e))
                    })?;
                }
                #[cfg(feature = "mysql")]
                DatabasePool::Mysql(pool) => {
                    let _conn = pool.get().map_err(|e| {
                        QuantumLogError::DatabaseError(format!("MySQL 连接测试失败: {}", e))
                    })?;
                }
                #[cfg(feature = "postgres")]
                DatabasePool::Postgres(pool) => {
                    let _conn = pool.get().map_err(|e| {
                        QuantumLogError::DatabaseError(format!("PostgreSQL 连接测试失败: {}", e))
                    })?;
                }
            }
            Ok::<(), QuantumLogError>(())
        })
        .await
        .map_err(|e| QuantumLogError::DatabaseError(format!("连接测试任务执行失败: {}", e)))?
    }
}

/// 为没有启用数据库特性的情况提供占位实现
#[cfg(not(feature = "database"))]
impl DatabaseSink {
    pub async fn new(_config: DatabaseSinkConfig) -> Result<Self> {
        Err(QuantumLogError::FeatureNotEnabled("database".to_string()))
    }

    pub fn spawn_task(
        self,
        _receiver: mpsc::Receiver<QuantumLogEvent>,
        _shutdown_signal: tokio::sync::broadcast::Receiver<()>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::spawn(async { Err(QuantumLogError::FeatureNotEnabled("database".to_string())) })
    }

    pub async fn test_connection(&self) -> Result<()> {
        Err(QuantumLogError::FeatureNotEnabled("database".to_string()))
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;
    use crate::config::DatabaseType;
    use tempfile::tempdir;

    #[tokio::test]
    #[cfg(feature = "sqlite")]
    async fn test_sqlite_database_sink_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let config = DatabaseSinkConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            db_type: DatabaseType::Sqlite,
            connection_string: format!("sqlite://{}", db_path.display()),
            schema_name: None,
            table_name: "quantum_logs".to_string(),
            batch_size: 100,
            connection_pool_size: 5,
            connection_timeout_ms: 5000,
            auto_create_table: true,
        };

        let sink = DatabaseSink::new(config).await;
        assert!(sink.is_ok());

        let sink = sink.unwrap();
        assert!(sink.test_connection().await.is_ok());
    }

    #[tokio::test]
    async fn test_log_batch_operations() {
        let mut batch = LogBatch::new();
        assert!(batch.is_empty());

        let entry = NewQuantumLogEntry::new(
            chrono::Utc::now().naive_utc(),
            "INFO".to_string(),
            "test".to_string(),
            "Test message".to_string(),
            1234,
            "thread-1".to_string(),
            "localhost".to_string(),
            "testuser".to_string(),
        );

        batch.add_entry(entry);
        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 1);

        batch.clear();
        assert!(batch.is_empty());
    }
}
