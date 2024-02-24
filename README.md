# git-query

`git-query` is a powerful command-line tool designed for querying and analyzing the commit history of a Git repository using SQL.

## Installation

Ensure that you have [Rust](https://www.rust-lang.org/tools/install) installed on your system. Then, install `git-query` using the following command:
```
cargo install git-query
```

## Usage

1. Open a terminal and navigate to the Git repository you want to query.
2. Run the git-query command:
   ```
   git-query
   ```
   This will initiate the program and execute an initial SQL query, displaying the latest commit for the repository.
3. You can then run SQL queries against the commits table. For example, to retrieve commits within a specific time range:
   ```
   SELECT * FROM commits WHERE date BETWEEN '2022-01-01 00:00:00 UTC' AND '2022-12-31 23:59:59 UTC';
   ```
4. To exit the program, simply enter the following command:
   ```
   exit
   ```

To load additional commit history from a commit that is not present (e.g., a commit in another branch), you can use the `traverse` command: `traverse <commit id>`.

If you only want commit history for a particular branch, navigate to that branch (using `git checkout`) before running `git-query`.

### Tables

See below for information on the SQL tables that can be queried, and the data within.

#### commits

* `id`: Commit id
* `author`: Author of the commit
* `date`: Datetime of the commit
* `message`: Commit message

#### branches

* `name`: Branch name
* `type`: Branch type (either remote or local)
* `head_commit_id`: HEAD commit id
* `head_commit_date`: Datetime of HEAD commit

#### tags

* `id`: Tag id
* `name`: The tag name
* `target_id`: The tag target id (e.g., commit id)
* `target_type`: The type of target (e.g., commit)
* `tagger`: Who created the tag
* `date`: Datetime of the tag
* `message`: The tag message. Any PGP signatures are removed

### Example queries

These queries use the [serde repo](https://github.com/serde-rs/serde).

#### Get most recent commit
```
>> SELECT * FROM commits ORDER BY date DESC LIMIT 1;
┌─────────┬──────────────┬─────────────────────────┬───────────────────────────┐
│ id      ┆ author       ┆ date                    ┆ message                   │
╞═════════╪══════════════╪═════════════════════════╪═══════════════════════════╡
│ 1d54973 ┆ David Tolnay ┆ 2024-02-13 03:49:34 UTC ┆ Merge pull request #2697  │
│         ┆              ┆                         ┆ from nyurik/format-str    │
│         ┆              ┆                         ┆                           │
│         ┆              ┆                         ┆ A few minor `write_str`   │
│         ┆              ┆                         ┆ optimizations             │
└─────────┴──────────────┴─────────────────────────┴───────────────────────────┘
Rows returned: 1
```

#### Get list of contributors
```
>> SELECT DISTINCT(author) FROM commits ORDER BY author LIMIT 5;
┌───────────────────┐
│ author            │
╞═══════════════════╡
│ Adam Crume        │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Adam H. Leventhal │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Alex              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Alex Crichton     │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Alex Shapiro      │
└───────────────────┘
Rows returned: 5
```

#### Get last commit by author
```
>> SELECT * FROM commits WHERE author = 'Adam Crume' ORDER BY date DESC LIMIT 1;
┌─────────┬────────────┬─────────────────────────┬─────────────────────────────┐
│ id      ┆ author     ┆ date                    ┆ message                     │
╞═════════╪════════════╪═════════════════════════╪═════════════════════════════╡
│ 05e931b ┆ Adam Crume ┆ 2018-06-03 04:11:42 UTC ┆ Update tests and use quote! │
│         ┆            ┆                         ┆ macro                       │
│         ┆            ┆                         ┆                             │
└─────────┴────────────┴─────────────────────────┴─────────────────────────────┘
Rows returned: 1
```

#### Get count of commits
```
>> SELECT COUNT(*) FROM commits;
┌──────────┐
│ COUNT(*) │
╞══════════╡
│ 3908     │
└──────────┘
Rows returned: 1
```

#### Get commits with message with specific pattern
```
>> SELECT * FROM commits WHERE message LIKE '%quote! macro%'
┌─────────┬────────────┬─────────────────────────┬─────────────────────────────┐
│ id      ┆ author     ┆ date                    ┆ message                     │
╞═════════╪════════════╪═════════════════════════╪═════════════════════════════╡
│ 05e931b ┆ Adam Crume ┆ 2018-06-03 04:11:42 UTC ┆ Update tests and use quote! │
│         ┆            ┆                         ┆ macro                       │
│         ┆            ┆                         ┆                             │
└─────────┴────────────┴─────────────────────────┴─────────────────────────────┘
Rows returned: 1
```

#### Get commits within a time range
```
>> SELECT * FROM commits WHERE date BETWEEN '2021-01-20 00:00:00 UTC' AND '2021-01-21 00:00:00 UTC';
┌─────────┬───────────────┬─────────────────────────┬──────────────────────────┐
│ id      ┆ author        ┆ date                    ┆ message                  │
╞═════════╪═══════════════╪═════════════════════════╪══════════════════════════╡
│ b276849 ┆ Jonas Bushart ┆ 2021-01-20 19:41:45 UTC ┆ Prevent panic when       │
│         ┆               ┆                         ┆ deserializing malformed  │
│         ┆               ┆                         ┆ Duration                 │
│         ┆               ┆                         ┆                          │
│         ┆               ┆                         ┆ std::time::Duration::new │
│         ┆               ┆                         ┆ can panic. There is no   │
│         ┆               ┆                         ┆ alternative non-panicing │
│         ┆               ┆                         ┆ constructor.             │
│         ┆               ┆                         ┆ Check the panic          │
│         ┆               ┆                         ┆ condition beforehand and │
│         ┆               ┆                         ┆ return an error instead  │
│         ┆               ┆                         ┆ of panicing.             │
│         ┆               ┆                         ┆                          │
│         ┆               ┆                         ┆ Fixes #1933              │
│         ┆               ┆                         ┆                          │
└─────────┴───────────────┴─────────────────────────┴──────────────────────────┘
Rows returned: 1
```

#### Get branches ordered by head commit date

```
>> SELECT * FROM branches ORDER BY head_commit_date DESC; 
┌───────────────┬────────┬────────────────┬─────────────────────────┐
│ name          ┆ type   ┆ head_commit_id ┆ head_commit_date        │
╞═══════════════╪════════╪════════════════╪═════════════════════════╡
│ master        ┆ local  ┆ 1d54973        ┆ 2024-02-13 03:49:34 UTC │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ origin/HEAD   ┆ remote ┆ 1d54973        ┆ 2024-02-13 03:49:34 UTC │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ origin/master ┆ remote ┆ 1d54973        ┆ 2024-02-13 03:49:34 UTC │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ origin/watt   ┆ remote ┆ af31449        ┆ 2022-07-02 00:56:18 UTC │
└───────────────┴────────┴────────────────┴─────────────────────────┘
Rows returned: 4
```

#### Get most recent tag

```
>> SELECT * FROM tags ORDER BY DATE DESC LIMIT 1;
┌─────────┬──────────┬───────────┬───────────┬───────────┬──────────┬──────────┐
│ id      ┆ name     ┆ target_id ┆ target_ty ┆ tagger    ┆ date     ┆ message  │
│         ┆          ┆           ┆ pe        ┆           ┆          ┆          │
╞═════════╪══════════╪═══════════╪═══════════╪═══════════╪══════════╪══════════╡
│ 9ed62c3 ┆ v1.0.196 ┆ ede9762   ┆ commit    ┆ David     ┆ 2024-01- ┆ Release  │
│         ┆          ┆           ┆           ┆ Tolnay    ┆ 26       ┆ 1.0.196  │
│         ┆          ┆           ┆           ┆           ┆ 22:00:35 ┆          │
│         ┆          ┆           ┆           ┆           ┆ UTC      ┆          │
└─────────┴──────────┴───────────┴───────────┴───────────┴──────────┴──────────┘
Rows returned: 1
```

#### Get most recent tag with commit info

```
>> SELECT commits.*, tags.id AS tag_id, tags.date AS tag_date, tags.message AS tag_message FROM commits INNER JOIN tags ON commits.id = tags.target_id ORDER BY tags.date DESC LIMIT 1;
┌─────────┬───────────┬───────────┬───────────┬─────────┬───────────┬──────────┐
│ id      ┆ author    ┆ date      ┆ message   ┆ tag_id  ┆ tag_date  ┆ tag_mess │
│         ┆           ┆           ┆           ┆         ┆           ┆ age      │
╞═════════╪═══════════╪═══════════╪═══════════╪═════════╪═══════════╪══════════╡
│ ede9762 ┆ David     ┆ 2024-01-2 ┆ Release   ┆ 9ed62c3 ┆ 2024-01-2 ┆ Release  │
│         ┆ Tolnay    ┆ 6         ┆ 1.0.196   ┆         ┆ 6         ┆ 1.0.196  │
│         ┆           ┆ 22:00:35  ┆           ┆         ┆ 22:00:35  ┆          │
│         ┆           ┆ UTC       ┆           ┆         ┆ UTC       ┆          │
└─────────┴───────────┴───────────┴───────────┴─────────┴───────────┴──────────┘
Rows returned: 1
```

### Tips

* Utilize standard SQL queries to extract valuable insights from your Git commit history.
* Experiment with different queries to tailor the results to your specific needs.
* Refer to the [SQLite documentation](https://www.sqlite.org/docs.html) for advanced SQL syntax.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.


