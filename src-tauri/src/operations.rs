#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReminderOperation {
    Probe,
    List,
    ListCategories,
    CreateCategory,
    UpdateCategory,
    DeleteCategory,
    Get,
    Create,
    Update,
    Delete,
}

impl ReminderOperation {
    pub const ALL: [Self; 10] = [
        Self::Probe,
        Self::List,
        Self::ListCategories,
        Self::CreateCategory,
        Self::UpdateCategory,
        Self::DeleteCategory,
        Self::Get,
        Self::Create,
        Self::Update,
        Self::Delete,
    ];

    pub const fn api_name(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::List => "list",
            Self::ListCategories => "list_categories",
            Self::CreateCategory => "create_category",
            Self::UpdateCategory => "update_category",
            Self::DeleteCategory => "delete_category",
            Self::Get => "get",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    pub const fn tool_name(self) -> &'static str {
        match self {
            Self::Probe => "samsung_reminders_status",
            Self::List => "samsung_reminders_list",
            Self::ListCategories => "samsung_reminder_categories_list",
            Self::CreateCategory => "samsung_reminder_category_create",
            Self::UpdateCategory => "samsung_reminder_category_update",
            Self::DeleteCategory => "samsung_reminder_category_delete",
            Self::Get => "samsung_reminders_get",
            Self::Create => "samsung_reminders_create",
            Self::Update => "samsung_reminders_update",
            Self::Delete => "samsung_reminders_delete",
        }
    }

    pub fn from_api_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.api_name() == name)
    }

    pub fn from_tool_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|operation| operation.tool_name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::ReminderOperation;

    #[test]
    fn every_operation_round_trips_through_both_names() {
        for operation in ReminderOperation::ALL {
            assert_eq!(
                ReminderOperation::from_api_name(operation.api_name()),
                Some(operation)
            );
            assert_eq!(
                ReminderOperation::from_tool_name(operation.tool_name()),
                Some(operation)
            );
        }
    }
}
