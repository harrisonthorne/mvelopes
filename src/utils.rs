pub fn remove_comments(s: &str) -> &str {
    // get indices
    let double_slash_index = s.find("//");
    let semicolon_index = s.find(';');

    // initialize cut_index with the index of the double slash
    let mut cut_index: Option<usize> = double_slash_index;

    if let Some(i) = semicolon_index {
        // if a semicolon exists...
        if let Some(j) = double_slash_index {
            // and the double_slash_index exists...
            if i < j {
                // and the semicolon comes before the slashes...
                cut_index = Some(i); // cut at the semicolon instead
            }
        } else {
            // but if no double slash exists...
            cut_index = Some(i); // use the semicolon no matter what
        }
    }

    // return a substring from the beginning up to the index at which the comment begins
    if let Some(i) = cut_index {
        &s[..i]
    } else {
        &s
    }
}
