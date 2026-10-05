def SERVICES = [
    'event-collector',
]

def REPO_SSH = 'ssh://git@ssh.bitbucket.juspay.net/picaf/events-collector.git'

pipeline {
    agent any

    environment {
        ECR_ACCOUNT_ID = '223655089699'
        ECR_REGION     = 'ap-south-1'
        REGISTRY       = "${ECR_ACCOUNT_ID}.dkr.ecr.${ECR_REGION}.amazonaws.com"
        IMAGE_REPO     = "${ECR_ACCOUNT_ID}.dkr.ecr.${ECR_REGION}.amazonaws.com/cdp"
    }

    stages {
        stage('Commit id') {
            steps {
                script {
                    env.COMMIT_ID = sh(
                        script: 'git rev-parse --short HEAD',
                        returnStdout: true,
                    ).trim()
                }
            }
        }

        stage('Version') {
            steps {
                script {
                    sh "git remote set-url origin ${REPO_SSH}"
                    sh 'git fetch --unshallow --tags || git fetch --tags'
                    env.NEXT_VERSION = sh(
                        script: 'bash scripts/next-version.sh',
                        returnStdout: true,
                    ).trim()
                }
            }
        }

        stage('ECR login') {
            steps {
                sh "aws ecr get-login-password --region ${ECR_REGION} | docker login --username AWS --password-stdin ${REGISTRY}"
            }
        }

        stage('Build workspace') {
            steps {
                sh "docker build -f crates/events-collector/Dockerfile --target builder ."
            }
        }

        stage('Build images') {
            steps {
                script {
                    for (String svc : SERVICES) {
                        sh "docker build -f crates/events-collector/Dockerfile -t ${env.IMAGE_REPO}:${svc}-${env.COMMIT_ID} ."
                        if (env.NEXT_VERSION?.trim()) {
                            sh "docker tag ${env.IMAGE_REPO}:${svc}-${env.COMMIT_ID} ${env.IMAGE_REPO}:${svc}-${env.NEXT_VERSION}"
                        }
                    }
                }
            }
        }

        stage('Push') {
            steps {
                script {
                    for (String svc : SERVICES) {
                        sh "docker push ${env.IMAGE_REPO}:${svc}-${env.COMMIT_ID}"
                        if (env.NEXT_VERSION?.trim()) {
                            sh "docker push ${env.IMAGE_REPO}:${svc}-${env.NEXT_VERSION}"
                        }
                    }
                }
            }
        }

        stage('Tag release') {
            steps {
                script {
                    if (env.NEXT_VERSION?.trim()) {
                        for (String svc : SERVICES) {
                            sh "git tag ${svc}-v${env.NEXT_VERSION}"
                            sh "git push origin ${svc}-v${env.NEXT_VERSION}"
                        }
                    }
                }
            }
        }
    }
}
