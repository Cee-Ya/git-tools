#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::path::Path;
use std::fs::File;
use std::io::*;
use std::process::Command;

// 项目配置
#[derive(Debug, Serialize, Deserialize)]
struct DefaultConfig {
    git: Option<GitConfig>,
    ai: Option<AiConfig>,
}

// git配置
#[derive(Debug, Serialize, Deserialize)]
struct GitConfig {
    path: Option<String>,
    branch: Option<String>,
}

// ai配置，目前只需要读取ai的key
#[derive(Debug, Serialize, Deserialize)]
struct AiConfig {
    key: Option<String>,
}

fn main() {
    let config : DefaultConfig;
    // 首先判断根路径下面是否存在default.toml文件
    if has_default_toml() {
        // 如果存在，则读取default.toml文件
        println!("default.toml文件存在");
        config = read_default_toml();
    } else {
        //  如果不存在，则开始用控制台交互的方式创建default.toml文件
        println!("default.toml文件不存在");
        config = create_default_toml();
    }
    // 使用配置信息执行命令
    get_git_log(&config);
}

// 获取git log （前提是已经打开了git仓库）
fn get_git_log(config: &DefaultConfig) {
    let git_config = config.git.as_ref().unwrap();
    let git_path = git_config.path.as_ref().unwrap();
    let git_branch = git_config.branch.as_ref().unwrap();
 
    // 先打开对应文件夹 切换分支为对应分支 更新代码 然后获取log
    let cmd = "git";
    let args = format!("-C {} checkout {}", git_path, git_branch);
    cmd_excute(cmd, &args);
    let args = format!("-C {} pull", git_path);
    cmd_excute(cmd, &args);
    // 获取最后一个tag到最后一个commit中间的所有commit
    // 命令为：git log $(git tag --sort=-creatordate | head -n 1)..HEAD 
    // cmd无法处理 | 符号，所以需要将命令拆分，代码处理head -n 1
    let args = format!("-C {} tag --sort=-creatordate", git_path);
    let tag = cmd_excute(cmd, &args);
    // 截取第一个tag
    let tag = tag.split_whitespace().next().unwrap();
    let args = format!("-C {} log {}..HEAD", git_path, tag);
    let log = cmd_excute(cmd, &args);
    println!("log: {}", log);
    let commits = log_split(&log);
    println!("commits: {:?}", commits);

}

// 将log分割成多个commit，只需要commit的内容
fn log_split(log: &str) -> Vec<String> {
    let mut commits = Vec::new();
    let mut commit = String::new();
    for line in log.lines() {
        if line.starts_with("commit") {
            if !commit.is_empty() {
                commits.push(commit);
                commit = String::new();
            }
        }
        commit.push_str(line);
        commit.push_str("\n");
    }
    commits.push(commit);
    commits
}

// 执行cmd命令
fn cmd_excute(cmd : &str, args : &str) -> String {
    // 打引入参
    println!("执行命令：{} {}",cmd, args);
    let output = Command::new(cmd)
        .args(args.split_whitespace())
        .output()
        .expect("failed to execute process");
    let result = String::from_utf8_lossy(&output.stdout);
    result.to_string()
}

// 创建default.toml文件
fn create_default_toml() -> DefaultConfig {
    // 创建一个DefaultConfig实例
    let mut default_config  = DefaultConfig {
        git: None,
        ai: None,
    };
    // 获取 git 配置
    println!(" Git 配置 ===> ");
    default_config.git = Some(get_git_config());

    // 获取 ai 配置
    println!(" AI 配置（如果不填写就是直接文字总结） ===> ");
    default_config.ai = Some(get_ai_config());

    println!("项目配置：{:?}", default_config);
    if let Err(e) = write_to_file(&default_config) {
        eprintln!("写入配置文件时出错：{}", e);
    } else {
        println!("配置已成功写入到 default.toml 文件中");
    }
    default_config
}

// 将DefaultConfig实例写入到default.toml文件中
fn write_to_file(config: &DefaultConfig) -> std::io::Result<()> {
    let toml_content = toml::to_string(config).unwrap();
    std::fs::write("default.toml", toml_content)?;
    Ok(())
}

// 获取Git配置
fn get_git_config() -> GitConfig {
    let mut git_config = GitConfig {
        path: None,
        branch: None,
    };

    print!("请输入 Git 仓库路径：");
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).expect("无法读取输入");
    git_config.path = Some(input.trim().to_string());

    input.clear();
    print!("请输入 Git 分支：");
    stdout().flush().unwrap();
    stdin().read_line(&mut input).expect("无法读取输入");
    git_config.branch = Some(input.trim().to_string());

    git_config
}

// 获取AI配置
fn get_ai_config() -> AiConfig {
    let mut ai_config = AiConfig {
        key: None,
    };

    print!("请输入 AI Key：");
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).expect("无法读取输入");
    ai_config.key = Some(input.trim().to_string());

    ai_config
}

// 读取default.toml文件
fn read_default_toml() -> DefaultConfig{
    // 1. 读取default.toml文件
    let file_path = "default.toml";
    let mut file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => panic!("no such file {} exception:{}", file_path, e)
    };
    // 2. 解析default.toml文件
    let mut str_val = String::new();
    match file.read_to_string(&mut str_val) {
        Ok(s) => s,
        Err(e) => panic!("Error Reading file: {}", e)
    };
    // 3. 返回DefaultConfig实例
    let config: DefaultConfig = toml::from_str(&str_val).unwrap();
    config
}

// 判断根路径下面是否存在default.toml文件
// 1. 判断根路径下面是否存在default.toml文件
// 2. 如果存在，则返回true
// 3. 如果不存在，则返回false
fn has_default_toml() -> bool {
    // 创建一个Path实例，表示default.toml文件的路径
    let path = Path::new("./default.toml");
    // 使用Path的exists方法判断文件是否存在
    path.exists()
}
