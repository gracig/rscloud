pub trait ResourceManager<Input, Output>: Send + Sync {
    fn lookup(&self, latest: &Output) -> Result<Option<Output>, ManagerError>;
    fn lookup_by_input(&self, input: &Input) -> Result<Option<Output>, ManagerError>;
    fn create(&self, input: &mut Input) -> Result<Output, ManagerError>;
    fn delete(&self, latest: &Output) -> Result<bool, ManagerError>;
    fn syncup(&self, latest: &Output, input: &mut Input) -> Result<Option<Output>, ManagerError>;
    fn ensure_absent(&self, latest: &Output) -> Result<bool, ManagerError> {
        match self.lookup(latest) {
            Ok(Some(_)) => self.delete(latest),
            Ok(None) => Ok(false),
            Err(err) => Err(err),
        }
    }
    fn ensure_present(
        &self,
        latest: Option<&Output>,
        input: &mut Input,
    ) -> Result<Output, ManagerError> {
        let latest = latest.and_then(|latest| self.lookup(latest).unwrap_or(None));
        let actual = self.lookup_by_input(input)?;
        match actual.or(latest) {
            Some(output) => self.syncup(&output, input).map(|response| match response {
                Some(output) => output,
                None => output,
            }),
            None => self.create(input),
        }
    }
}
#[derive(Debug, Clone)]
pub enum ManagerError {
    DeleteFail(String),
    CreateFail(String),
    UpdateFail(String),
    LookupFail(String),
    CannotSyncWithoutRecreate(String),
}
