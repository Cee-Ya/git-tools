#[macro_use]
extern crate serde_derive;
extern crate toml;

use std::path::Path;
use std::fs::File;
use std::io::{stdin, stdout, Read, Write};
use std::process::Command;
use std::str;
use regex::Regex;
use chatgpt::prelude::{ChatGPT, ModelConfigurationBuilder,ChatGPTEngine,Url};
use chatgpt::types::CompletionResponse;

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
    url: Option<String>,
}


const DEFAULT_OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
// 一个小功能，通过获取git log，然后通过gpt生成一个版本更新的总结
// 协助开发者创建版本发布的描述
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>>{

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
    let (git_commint_log, tag)  = get_git_log(&config);

    if git_commint_log.is_empty() {
        println!("没有新的提交记录，无需生成版本更新的总结");
        // 程序阻塞 ,不退出
        let _ = Command::new("cmd.exe").arg("/c").arg("pause").status();
        return Ok(())
    }

    // 使用gpt生成版本更新的总结
    let ai_config = config.ai.as_ref().unwrap();
    let ai_key = ai_config.key.as_ref().unwrap();
    let ai_url = ai_config.url.as_ref().unwrap();
    if ai_key.is_empty() {
        println!("AI Key 为空，无法使用AI功能");
        // 机器总结
        let mut message = String::from("本次git提交记录如下：\n");
        for commit in git_commint_log {
            message.push_str(&commit);
            message.push_str("\n");
        }
        
        println!("上次发布版本号为{}，本次发布版本的内容如下：\n{}", tag, message);
        // 程序阻塞 ,不退出
        let _ = Command::new("cmd.exe").arg("/c").arg("pause").status();
        return Ok(())
    }
    let parsed_url = Url::parse(ai_url)?;
    let client = ChatGPT::new_with_config(
        ai_key,
        ModelConfigurationBuilder::default()
            .api_url(parsed_url)
            .temperature(1.0)
            .engine(ChatGPTEngine::Gpt35Turbo)
            .build()
            .unwrap(),
    )?;
    // 组合git_commint_log,让gpt生成一个版本更新的总结
    let mut message = String::from("本次git提交记录如下：\n");
    for commit in git_commint_log {
        message.push_str(&commit);
        message.push_str("\n");
    }
    message.push_str("请为本次生成一个git创建代码版本发布的描述：\n");
    message.push_str("返回格式如下：\n");
    message.push_str("1. 本次版本新增了如下功能：\n");
    message.push_str(" a. 新增了xxxx 功能 \n");
    message.push_str("2. 本次版本修复了如下bug：\n");
    message.push_str(" a. 修复了xxxx bug \n");
    message.push_str("3. 本次版本优化了如下功能：\n");
    message.push_str(" a. 优化了xxxx 功能 \n");
    message.push_str("...\n");
    message.push_str("请按照上面的格式返回版本更新的总结：\n");

    let start_time = std::time::Instant::now();
    let response: CompletionResponse = client.send_message(message).await?;
    println!("生成版本更新的总结成功，耗时：{:?}毫秒", start_time.elapsed().as_millis());

    println!("上次发布版本号为{}，本次发布版本的内容如下：\n{}", tag, response.message().content);


    // 程序阻塞 ,不退出
    let _ = Command::new("cmd.exe").arg("/c").arg("pause").status();
    Ok(())

}

// 获取git log （前提是已经打开了git仓库）
fn get_git_log(config: &DefaultConfig) -> (Vec<String>,String) {
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
    let tag = tag.split_whitespace().next().expect("没有tag");
    let args = format!("-C {} log {}..HEAD", git_path, tag);
    let log = cmd_excute(cmd, &args);
    // 内容格式为：
    let commits = log_split(&log);
    (commits, tag.to_string())

}

// 将log分割成多个commit，只提取commit的信息，其他的信息不要
fn log_split(log: &str) -> Vec<String> {
    let mut commits = Vec::new();
    let re = Regex::new(r"Date:.*\n\n\s*(.*)").unwrap();
    for cap in re.captures_iter(log) {
        let commit = cap.get(1).map_or("", |m| m.as_str());
        commits.push(commit.to_string());
    }
    commits
}

// 执行cmd命令
fn cmd_excute(cmd : &str, args : &str) -> String {
    // 打引入参
    println!("执行命令：{} {}",cmd, args);
    let output = Command::new(cmd)
        .args(args.split_whitespace())
        .output()
        .expect("cmd 执行失败");
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
    println!(" [Git 配置] ");
    default_config.git = Some(get_git_config());

    // 获取 ai 配置
    println!(" [AI 配置（如果不填写就是直接文字总结）] ");
    default_config.ai = Some(get_ai_config());

    if let Err(e) = write_to_file(&default_config) {
        panic!("写入配置文件失败: {}", e)
    } else {
        println!("配置已成功写入到 default.toml 文件中");
    }
    default_config
}

// 将DefaultConfig实例写入到default.toml文件中
fn write_to_file(config: &DefaultConfig) -> std::io::Result<()> {
    let toml_content = toml::to_string(config).expect("无法序列化为toml");
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
    stdout().flush().expect("无法刷新输出");
    let mut input = String::new();
    stdin().read_line(&mut input).expect("无法读取输入");
    git_config.path = match input.trim() {
        "" => panic!("Git 仓库路径不能为空"),
        _ => Some(input.trim().to_string()),
    };

    // 判断该路径是否是一个文件夹
    // let path = Path::new(&git_config.path.as_ref().unwrap());
    // if !path.is_dir() {
    //     panic!("{} 不是一个文件夹", path.display());
    // }
    // 判断该文件夹下是否存在.git文件夹
    // let git_path = path.join(".git");
    // if !git_path.is_dir() {
    //     panic!("{} 不是一个git仓库", path.display());
    // }

    input.clear();
    print!("请输入 Git 分支：");
    stdout().flush().expect("无法刷新输出");
    stdin().read_line(&mut input).expect("无法读取输入");
    git_config.branch = match input.trim() {
        "" => panic!("Git 分支不能为空"),
        _ => Some(input.trim().to_string()),
    };

    git_config
}

// 获取AI配置
fn get_ai_config() -> AiConfig {
    let mut ai_config = AiConfig {
        key: None,
        url: Some(DEFAULT_OPENAI_API_URL.to_string()),
    };

    print!("请输入 AI Key：");
    stdout().flush().unwrap();
    let mut input = String::new();
    stdin().read_line(&mut input).expect("无法读取输入");
    ai_config.key = Some(input.trim().to_string());
    // 如果key为空，则不需要修改url
    if ai_config.key.as_ref().unwrap().is_empty() {
        return ai_config;
    }
    print!("AI URL 默认为 {}，是否需要修改？(y/n)：", DEFAULT_OPENAI_API_URL);
    stdout().flush().unwrap();
    input.clear();
    stdin().read_line(&mut input).expect("无法读取输入");
    if input.trim() == "y" {
        input.clear();
        print!("请输入 AI URL：");
        stdout().flush().unwrap();
        stdin().read_line(&mut input).expect("无法读取输入");
        ai_config.url = Some(input.trim().to_string());
    }

    ai_config
}

// 读取default.toml文件
fn read_default_toml() -> DefaultConfig{
    // 读取default.toml文件
    let file_path = "default.toml";
    let mut file = match File::open(file_path) {
        Ok(f) => f,
        Err(e) => panic!("no such file {} exception:{}", file_path, e)
    };
    // 解析default.toml文件
    let mut str_val = String::new();
    match file.read_to_string(&mut str_val) {
        Ok(s) => s,
        Err(e) => panic!("Error Reading file: {}", e)
    };
    // 返回DefaultConfig实例
    let config: DefaultConfig = toml::from_str(&str_val).expect("无法解析toml文件");
    // 判断config.git.path是否为空
    if config.git.as_ref().unwrap().path.is_none() {
        panic!("default.toml文件中的git.path为空");
    }
    // // 判断config.git.path是否是一个文件夹
    // let path = Path::new(config.git.as_ref().unwrap().path.as_ref().unwrap());
    // if !path.is_dir() {
    //     panic!("{} 不是一个文件夹", path.display());
    // }
    // // 判断config.git.path是否是一个git仓库
    // let git_path = path.join(".git");
    // if !git_path.is_dir() {
    //     panic!("{} 不是一个git仓库", path.display());
    // }
    // // 判断config.git.branch是否为空
    // if config.git.as_ref().unwrap().branch.is_none() {
    //     panic!("default.toml文件中的git.branch为空");
    // }
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
