use std::time::{Duration, SystemTime};
use std::{collections::HashMap, future::Future, sync::Arc};

use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tracing::{event, Level};

use crate::error::Error;

static TIME_TO_KEEP: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Clone, Debug)]
pub enum TaskResult {
    Processing,
    Error,
    Success,
}

#[derive(Clone, Debug)]
struct Task {
    result: TaskResult,
    created_at: SystemTime,
}

impl From<TaskResult> for Task {
    fn from(result: TaskResult) -> Self {
        Self {
            result,
            created_at: SystemTime::now(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TaskList {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    last_cleanup: Arc<Mutex<SystemTime>>,
}

impl TaskList {
    async fn insert(&self, id: &str) {
        self.update(id, TaskResult::Processing).await;
    }

    async fn remove_old_entries(&self) {
        let now = SystemTime::now();
        let needed = {
            let mut last = self.last_cleanup.lock().await;
            let r = now.duration_since(*last).unwrap_or_default() >= TIME_TO_KEEP;
            if r {
                *last = now;
            }
            r
        };
        if needed {
            self.tasks.write().await.retain(|_, task| {
                now.duration_since(task.created_at).unwrap_or_default() < TIME_TO_KEEP
            });
        }
    }

    pub async fn get(&self, id: &str) -> Result<TaskResult, Error> {
        let tasks = self.tasks.read().await;
        tasks
            .get(id)
            .ok_or(Error::NotFound)
            .map(|task| task.result.clone())
    }

    pub async fn update(&self, id: &str, result: TaskResult) {
        let task = Task {
            result,
            created_at: SystemTime::now(),
        };
        self.tasks.write().await.insert(id.into(), task);
    }

    pub async fn spawn_task(
        &self,
        id: String,
        future: impl Future<Output = Result<(), Error>> + Send + 'static,
    ) {
        self.insert(&id).await;
        let tasks = self.clone();

        async fn spawned_task(
            future: impl Future<Output = Result<(), Error>> + Send,
            id: String,
            tasks: TaskList,
        ) {
            let result = future.await;

            let task_result = match result {
                Ok(_) => {
                    event!(Level::INFO, "Task completed");
                    TaskResult::Success
                }
                Err(e) => {
                    event!(Level::ERROR, "Task failed: {:?}", e);
                    TaskResult::Error
                }
            };
            tasks.update(&id, task_result).await;
            tasks.remove_old_entries().await;
        }

        tokio::spawn(spawned_task(future, id, tasks));

        // tokio::spawn(async move {
        //     let result = match f().await {
        //         Ok(_) => TaskResult::Success,
        //         Err(_) => TaskResult::Error,
        //     };
        //     tasks.update(&id, result).await;
        //     tasks.remove_old_entries().await;
        // });
    }
}

impl Default for TaskList {
    fn default() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            last_cleanup: Arc::new(Mutex::new(SystemTime::now())),
        }
    }
}
