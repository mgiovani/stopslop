/// # Examples
///
/// ```
/// let items = vec![1, 2, 3];
/// // ... item processing ...
/// let sum: i32 = items.iter().sum();
/// assert_eq!(sum, 6);
/// ```
pub fn sum(items: &[i32]) -> i32 {
    items.iter().sum()
}
