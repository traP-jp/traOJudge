CREATE TABLE IF NOT EXISTS `users` (
	`user_id` INT AUTO_INCREMENT PRIMARY KEY,
	`username` VARCHAR(255) NOT NULL,
	`traq_id` VARCHAR(255),
	`github_id` VARCHAR(255),
	`icon_url` VARCHAR(255),
	`submit_count` INT DEFAULT 0,
	`post_problem_count` INT DEFAULT 0,
	`role` INT DEFAULT 0,
	`created_at` DATETIME DEFAULT CURRENT_TIMESTAMP,
	`updated_at` DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
	`email` VARCHAR(255),
	`hashed_pass` VARCHAR(255)
);

CREATE TABLE IF NOT EXISTS `mail_verifications` (
	`email_id` INT AUTO_INCREMENT PRIMARY KEY,
	`email` VARCHAR(255) NOT NULL,
	`token` VARCHAR(255) NOT NULL,
	`created_at` DATETIME DEFAULT CURRENT_TIMESTAMP
);



CREATE TABLE IF NOT EXISTS `normal_problems` (
	`problem_id` INT AUTO_INCREMENT PRIMARY KEY,
	`auther_id` INT NOT NULL,
	`statement` TEXT NOT NULL,
	`time_limit` INT NOT NULL,
	`memory_limit` INT NOT NULL,
	`difficulty` INT NOT NULL,
	`is_public` BOOLEAN DEFAULT FALSE,
	`judgecode_path` VARCHAR(255) NOT NULL,
    `publish_time` DATETIME DEFAULT CURRENT_TIMESTAMP,
    `updated_time` DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS `testcases` (
	`testcase_id` INT AUTO_INCREMENT PRIMARY KEY,
	`problem_id` INT NOT NULL,
	`testcase_name` VARCHAR(255) NOT NULL,
	`testcase_path` VARCHAR(255) NOT NULL
);

CREATE TABLE IF NOT EXISTS `editiorials` (
	`editorial_id` INT AUTO_INCREMENT PRIMARY KEY,
	`problem_id` INT NOT NULL,
	`auther_id` INT NOT NULL,
	`statement` TEXT NOT NULL,
	`publish_time` DATETIME DEFAULT CURRENT_TIMESTAMP,
	`updated_time` DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS `submissions` (
	`submission_id` INT AUTO_INCREMENT PRIMARY KEY,
	`problem_id` INT NOT NULL,
	`auther_id` INT NOT NULL,
	`language_id` INT NOT NULL,
	`code` TEXT NOT NULL,
	`judge_status` VARCHAR(4) NOT NULL,
	`total_score` BIGINT NOT NULL,
	`max_time` INT NOT NULL,
	`max_memory` INT NOT NULL,
	`submitted_at` DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS `submission_testcases` (
	`testcase_id` INT AUTO_INCREMENT PRIMARY KEY,
	`submission_id` INT NOT NULL,
	`testcase_name` VARCHAR(255) NOT NULL,
	`judge_status` VARCHAR(4) NOT NULL,
	`score` BIGINT NOT NULL,
	`time` INT NOT NULL,
	`memory` INT NOT NULL
);